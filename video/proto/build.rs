use std::collections::{HashMap, HashSet};
use std::io::BufRead;

use quote::quote;
use walkdir::WalkDir;

const PROTO_DIR: &str = env!("CARGO_MANIFEST_DIR");

#[derive(Debug, Default)]
struct Tree {
	children: HashMap<String, Tree>,
	absolute: String,
	leaf: bool,
}

fn generate_modules(root: &HashMap<String, Tree>) -> proc_macro2::TokenStream {
	let modules = root
		.iter()
		.map(|(part, tree)| {
			let children = generate_modules(&tree.children);
			let leaf = if tree.leaf {
				let absolute = &tree.absolute;
				quote! {
					::tonic::include_proto!(#absolute);
				}
			} else {
				quote! {}
			};

			let part = syn::Ident::new(part, proc_macro2::Span::call_site());

			quote! {
				pub mod #part {
					#leaf
					#children
				}
			}
		})
		.collect::<Vec<_>>();

	quote! {
		#(#modules)*
	}
}

fn main() {
	let mut config = prost_build::Config::new();

	config.protoc_arg("--experimental_allow_proto3_optional");
	config.bytes(["."]);

	let proto_files = WalkDir::new(".")
		.follow_links(true)
		.into_iter()
		.filter_map(|e| e.ok())
		.filter(|e| e.path().extension() == Some(std::ffi::OsStr::new("proto")))
		.map(|e| {
			let path = e.path().canonicalize().unwrap().display().to_string();
			println!("cargo:rerun-if-changed={}", path);
			path
		})
		.collect::<Vec<_>>();

	let mut root_tree = Tree::default();

	proto_files
		.iter()
		.filter_map(|f| {
			let file = std::fs::File::open(f).unwrap();
			std::io::BufReader::new(file).lines().map_while(Result::ok).find_map(|l| {
				l.strip_prefix("package ")
					.and_then(|l| l.strip_suffix(';'))
					.map(|l| l.to_string())
			})
		})
		.collect::<HashSet<_>>()
		.into_iter()
		.for_each(|p| {
			let mut tree = &mut root_tree;
			for part in p.split('.') {
				let absolute = format!("{}.{}", tree.absolute, part);
				tree = tree.children.entry(part.to_string()).or_insert_with(Tree::default);
				tree.absolute = absolute.trim_start_matches('.').to_string();
			}

			tree.leaf = true;
		});

	std::fs::write(
		std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("module.rs"),
		prettyplease::unparse(&syn::parse2(generate_modules(&root_tree.children)).unwrap()),
	)
	.unwrap();

	tonic_build::configure()
		.compile_with_config(config, &proto_files, &[PROTO_DIR])
		.unwrap();
}
