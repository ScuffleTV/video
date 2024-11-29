use darling::ast::NestedMeta;
use darling::FromMeta;
use quote::ToTokens;
use syn::parse::Parse;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::Token;

#[derive(Debug, FromMeta)]
#[darling(default)]
#[derive(Default)]
struct ModuleOptions {
	crate_path: Option<syn::Path>,
	rename: Option<syn::LitStr>,
}

impl Parse for ModuleOptions {
	fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
		if input.is_empty() {
			Ok(ModuleOptions::default())
		} else {
			let meta_list = Punctuated::<NestedMeta, Token![,]>::parse_terminated(input)?
				.into_iter()
				.collect::<Vec<_>>();

			Ok(ModuleOptions::from_list(&meta_list)?)
		}
	}
}

#[derive(Debug, FromMeta)]
#[darling(default)]
#[derive(Default)]
struct Options {
	crate_path: Option<syn::Path>,
	builder: Option<syn::Expr>,
	unit: Option<syn::LitStr>,
	rename: Option<syn::LitStr>,
}

impl Parse for Options {
	fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
		if input.is_empty() {
			Ok(Options::default())
		} else {
			let meta_list = Punctuated::<NestedMeta, Token![,]>::parse_terminated(input)?
				.into_iter()
				.collect::<Vec<_>>();

			Ok(Options::from_list(&meta_list)?)
		}
	}
}

enum ModuleItem {
	Other(syn::Item),
	Function(proc_macro2::TokenStream),
}

struct FunctionAttrs {
	cfg_attrs: Vec<syn::Attribute>,
	docs: Vec<syn::LitStr>,
	options: Options,
}

impl FunctionAttrs {
	fn from_attrs(attrs: Vec<syn::Attribute>) -> syn::Result<Self> {
		let (cfg_attrs, others): (Vec<_>, Vec<_>) = attrs.into_iter().partition(|attr| attr.path().is_ident("cfg"));

		let (doc_attrs, others): (Vec<_>, Vec<_>) = others.into_iter().partition(|attr| attr.path().is_ident("doc"));

		Ok(FunctionAttrs {
			cfg_attrs,
			docs: doc_attrs
				.into_iter()
				.map(|attr| match attr.meta {
					syn::Meta::NameValue(syn::MetaNameValue {
						value: syn::Expr::Lit(syn::ExprLit {
							lit: syn::Lit::Str(lit), ..
						}),
						..
					}) => Ok(lit),
					_ => Err(syn::Error::new_spanned(attr, "expected string literal")),
				})
				.collect::<Result<_, _>>()?,
			options: {
				let mut meta = Vec::new();
				for attr in &others {
					if attr.path().is_ident("metrics") {
						match &attr.meta {
							syn::Meta::List(syn::MetaList { tokens, .. }) => {
								meta.extend(NestedMeta::parse_meta_list(tokens.clone())?);
							}
							_ => return Err(syn::Error::new_spanned(attr, "expected list")),
						}
					}
				}

				Options::from_list(&meta)?
			},
		})
	}
}

pub struct Function {
	vis: syn::Visibility,
	fn_token: Token![fn],
	ident: syn::Ident,
	args: syn::punctuated::Punctuated<FnArg, Token![,]>,
	arrow_token: Token![->],
	ret: syn::Type,
	attrs: FunctionAttrs,
}

impl Parse for Function {
	fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
		let attrs = input.call(syn::Attribute::parse_outer)?;
		let vis = input.parse()?;
		let fn_token = input.parse()?;
		let ident = input.parse()?;
		let args_content;
		let _paren = syn::parenthesized!(args_content in input);
		let args = args_content.parse_terminated(FnArg::parse, Token![,])?;
		let arrow_token = input.parse()?;
		let ret = input.parse()?;
		input.parse::<Token![;]>()?;

		Ok(Function {
			vis,
			fn_token,
			ident,
			args,
			arrow_token,
			ret,
			attrs: FunctionAttrs::from_attrs(attrs)?,
		})
	}
}

struct FnArg {
	cfg_attrs: Vec<syn::Attribute>,
	other_attrs: Vec<syn::Attribute>,
	options: FnArgOptions,
	ident: syn::Ident,
	colon_token: Token![:],
	ty: syn::Type,
	struct_ty: StructTy,
}

#[derive(Debug, FromMeta)]
#[darling(default)]
#[derive(Default)]
struct FnArgOptions {
	rename: Option<syn::LitStr>,
}

impl Parse for FnArgOptions {
	fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
		if input.is_empty() {
			Ok(FnArgOptions::default())
		} else {
			let meta_list = Punctuated::<NestedMeta, Token![,]>::parse_terminated(input)?
				.into_iter()
				.collect::<Vec<_>>();

			Ok(FnArgOptions::from_list(&meta_list)?)
		}
	}
}

enum StructTy {
	Clone(syn::Type),
	Into(syn::Type),
	Raw(syn::Type),
	Str(syn::Type),
}

impl StructTy {
	fn ty(&self) -> &syn::Type {
		match self {
			StructTy::Clone(ty) => ty,
			StructTy::Into(ty) => ty,
			StructTy::Raw(ty) => ty,
			StructTy::Str(ty) => ty,
		}
	}
}

fn type_to_struct_type(ty: syn::Type) -> syn::Result<StructTy> {
	match ty.clone() {
		syn::Type::Reference(syn::TypeReference { elem, lifetime, .. }) => {
			if lifetime.is_some_and(|lifetime| lifetime.ident == "static") {
				return Ok(StructTy::Raw(ty));
			}

			if let syn::Type::Path(syn::TypePath { path, .. }) = &*elem {
				if path.is_ident("str") {
					return Ok(StructTy::Str(
						syn::parse_quote_spanned! { ty.span() => ::std::sync::Arc<#path> },
					));
				}
			}

			Ok(StructTy::Clone(*elem))
		}
		// Also support impl types
		syn::Type::ImplTrait(impl_trait) => impl_trait
			.bounds
			.iter()
			.find_map(|bound| match bound {
				syn::TypeParamBound::Trait(syn::TraitBound {
					path: syn::Path { segments, .. },
					..
				}) => {
					let first_segment = segments.first()?;
					if first_segment.ident != "Into" {
						return None;
					}

					let args = match first_segment.arguments {
						syn::PathArguments::AngleBracketed(ref args) => args.args.clone(),
						_ => return None,
					};

					if args.len() != 1 {
						return None;
					}

					match &args[0] {
						syn::GenericArgument::Type(ty) => Some(StructTy::Into(ty.clone())),
						_ => None,
					}
				}
				_ => None,
			})
			.ok_or_else(|| syn::Error::new_spanned(impl_trait, "only impl Into<T> trait bounds are supported")),
		_ => Ok(StructTy::Raw(ty)),
	}
}

impl Parse for FnArg {
	fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
		let attrs = input.call(syn::Attribute::parse_outer)?;
		let ident = input.parse()?;
		let colon_token = input.parse()?;
		let ty: syn::Type = input.parse()?;
		let struct_ty = type_to_struct_type(ty.clone())?;

		let (cfg_attrs, other_attrs): (Vec<_>, Vec<_>) = attrs.into_iter().partition(|attr| attr.path().is_ident("cfg"));

		let (metric_attrs, other_attrs): (Vec<_>, Vec<_>) =
			other_attrs.into_iter().partition(|attr| attr.path().is_ident("metrics"));

		let mut meta = Vec::new();
		for attr in &metric_attrs {
			match &attr.meta {
				syn::Meta::List(syn::MetaList { tokens, .. }) => {
					meta.extend(NestedMeta::parse_meta_list(tokens.clone())?);
				}
				_ => return Err(syn::Error::new_spanned(attr, "expected list")),
			}
		}

		let options = FnArgOptions::from_list(&meta)?;

		Ok(FnArg {
			ident,
			cfg_attrs,
			other_attrs,
			options,
			colon_token,
			ty,
			struct_ty,
		})
	}
}

impl ToTokens for FnArg {
	fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
		for attr in &self.cfg_attrs {
			attr.to_tokens(tokens);
		}

		for attr in &self.other_attrs {
			attr.to_tokens(tokens);
		}

		self.ident.to_tokens(tokens);
		self.colon_token.to_tokens(tokens);
		self.ty.to_tokens(tokens);
	}
}

fn metric_function(
	input: proc_macro2::TokenStream,
	module_name: Option<&str>,
	module_options: &ModuleOptions,
) -> syn::Result<proc_macro2::TokenStream> {
	let item = syn::parse2::<Function>(input)?;

	let crate_path = item
		.attrs
		.options
		.crate_path
		.clone()
		.or(module_options.crate_path.clone())
		.unwrap_or_else(|| syn::parse_quote!(::scuffle_metrics));

	let ident = &item.ident;
	let vis = &item.vis;
	let ret = &item.ret;

	let const_assert_ret = quote::quote_spanned! { ret.span() =>
		__assert_impl_collector::<#ret>();
	};

	let options = &item.attrs.options;
	let name = &options.rename;
	let attrs = &item.attrs.cfg_attrs;
	let docs = &item.attrs.docs;
	let fn_token = &item.fn_token;
	let arrow_token = &item.arrow_token;
	let args = &item.args;

	let collect_args = args
		.iter()
		.map(|arg| {
			let ident = &arg.ident;
			let ty = &arg.struct_ty.ty();

			let arg_tokens = match &arg.struct_ty {
				StructTy::Clone(_) => quote::quote! {
					::core::clone::Clone::clone(#ident)
				},
				StructTy::Into(_) => quote::quote! {
					::core::convert::Into::into(#ident)
				},
				StructTy::Raw(_) => quote::quote! {
					#ident
				},
				StructTy::Str(_) => quote::quote! {
					::std::sync::Arc::from(#ident)
				},
			};

			let name = if let Some(name) = &arg.options.rename {
				name.value()
			} else {
				ident.to_string()
			};

			quote::quote! {
				let #ident: #ty = #arg_tokens;
				if let Some(#ident) = #crate_path::to_value!(#ident) {
					___args.push(#crate_path::opentelemetry::KeyValue::new(
						#crate_path::opentelemetry::Key::from_static_str(#name),
						#ident,
					));
				}
			}
		})
		.collect::<Vec<_>>();

	let name = if let Some(name) = name {
		name.value()
	} else {
		ident.to_string()
	};

	let name = if let Some(module_name) = module_name {
		format!("{module_name}_{name}")
	} else {
		name
	};

	let make_metric = {
		let help = docs.iter().map(|doc| doc.value()).collect::<Vec<_>>();
		let help = help
			.iter()
			.map(|help| ::core::primitive::str::trim_end_matches(help.trim(), "."))
			.filter(|help| !help.is_empty())
			.collect::<Vec<_>>();

		let help = if help.is_empty() {
			quote::quote! {}
		} else {
			let help = help.join(" ");
			quote::quote! {
				builder = builder.with_description(#help);
			}
		};

		let unit = if let Some(unit) = &options.unit {
			quote::quote! {
				builder = builder.with_unit(#unit);
			}
		} else {
			quote::quote! {}
		};

		let builder = if let Some(expr) = &options.builder {
			quote::quote! {
				{ #expr }
			}
		} else {
			quote::quote! {
				|builder| { builder }
			}
		};

		quote::quote! {
			let callback = #builder;

			let meter = #crate_path::opentelemetry::global::meter_with_scope(
				#crate_path::opentelemetry::InstrumentationScope::builder(env!("CARGO_PKG_NAME"))
					.with_version(env!("CARGO_PKG_VERSION"))
					.build()
			);

			#[allow(unused_mut)]
			let mut builder = <#ret as #crate_path::collector::IsCollector>::builder(&meter, #name);

			#help

			#unit

			callback(builder).build()
		}
	};

	let assert_collector_fn = quote::quote! {
		const fn __assert_impl_collector<T: #crate_path::collector::IsCollector>() {}
	};

	let fn_body = quote::quote! {
		#assert_collector_fn
		#const_assert_ret

		#[allow(unused_mut)]
		let mut ___args = Vec::new();

		#(#collect_args)*

		static __COLLECTOR: std::sync::OnceLock<#ret> = std::sync::OnceLock::new();

		let collector = __COLLECTOR.get_or_init(|| { #make_metric });

		#crate_path::collector::Collector::new(___args, collector)
	};

	Ok(quote::quote! {
		#(#attrs)*
		#(#[doc = #docs])*
		#vis #fn_token #ident(#args) #arrow_token #crate_path::collector::Collector<'static, #ret> {
			#fn_body
		}
	})
}

pub fn metrics_impl(args: proc_macro::TokenStream, input: proc_macro::TokenStream) -> syn::Result<proc_macro2::TokenStream> {
	let module = match syn::parse::<syn::Item>(input)? {
		syn::Item::Mod(module) => module,
		syn::Item::Verbatim(tokens) => return metric_function(tokens, None, &Default::default()),
		item => return Err(syn::Error::new_spanned(item, "expected module or bare function")),
	};

	let args = syn::parse::<ModuleOptions>(args)?;

	let ident = &module.ident;

	let module_name = if let Some(rename) = args.rename.as_ref() {
		rename.value()
	} else {
		ident.to_string()
	};
	let vis = &module.vis;

	let items = module
		.content
		.into_iter()
		.flat_map(|(_, item)| item)
		.map(|item| match item {
			syn::Item::Verbatim(verbatim) => metric_function(verbatim, Some(&module_name), &args).map(ModuleItem::Function),
			item => Ok(ModuleItem::Other(item)),
		})
		.collect::<syn::Result<Vec<_>>>()?;

	let items = items.into_iter().map(|item| match item {
		ModuleItem::Other(item) => item,
		ModuleItem::Function(item) => syn::Item::Verbatim(item),
	});

	Ok(quote::quote! {
		#vis mod #ident {
			#(#items)*
		}
	})
}
