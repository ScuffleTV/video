#![allow(unused)]

use darling::ast::NestedMeta;
use darling::FromMeta;
use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{parse_macro_input, parse_quote, Ident, Token};

struct Main {
	options: ParseArgs,
	entry: syn::Type,
	braced: syn::token::Brace,
	items: Vec<Item>,
}

impl syn::parse::Parse for Main {
	fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
		let content;

		Ok(Main {
			options: ParseArgs::from_attrs(syn::Attribute::parse_outer(input)?)?,
			entry: input.parse()?,
			braced: syn::braced!(content in input),
			items: content.parse_terminated(Item::parse, Token![,])?.into_iter().collect(),
		})
	}
}

#[derive(Debug)]
struct Item {
	cfg_attrs: Vec<syn::Attribute>,
	expr: syn::Expr,
	item_kind: ItemKind,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ItemKind {
	Service,
}

impl syn::parse::Parse for Item {
	fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
		// Parse any attributes before the item
		let attrs = syn::Attribute::parse_outer(input)?;
		let mut cfg_attrs = Vec::new();
		let mut item_kind = None;

		for attr in attrs {
			if attr.path().is_ident("cfg") {
				cfg_attrs.push(attr);
			} else {
				return Err(syn::Error::new_spanned(attr, "unknown attribute"));
			}
		}

		Ok(Item {
			cfg_attrs,
			expr: input.parse()?,
			item_kind: item_kind.unwrap_or(ItemKind::Service),
		})
	}
}

#[derive(Debug, darling::FromMeta)]
#[darling(default)]
struct ParseArgs {
	crate_path: syn::Path,
}

impl ParseArgs {
	fn from_attrs(attrs: Vec<syn::Attribute>) -> syn::Result<Self> {
		let mut meta = Vec::new();

		for attr in attrs {
			if attr.path().is_ident("bootstrap") {
				match attr.meta {
					syn::Meta::List(list) => {
						let meta_list =
							syn::parse::Parser::parse2(Punctuated::<NestedMeta, Token![,]>::parse_terminated, list.tokens)?;
						meta.extend(meta_list);
					}
					_ => {
						return Err(syn::Error::new_spanned(attr, "expected list, #[bootstrap(...)]"));
					}
				}
			} else {
				return Err(syn::Error::new_spanned(attr, "unknown attribute"));
			}
		}

		Ok(Self::from_list(&meta)?)
	}
}

impl Default for ParseArgs {
	fn default() -> Self {
		Self {
			crate_path: syn::parse_str("::scuffle_bootstrap").unwrap(),
		}
	}
}

impl syn::parse::Parse for ParseArgs {
	fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
		if input.is_empty() {
			Ok(ParseArgs::default())
		} else {
			let meta_list = input
				.parse_terminated(NestedMeta::parse, Token![,])?
				.into_iter()
				.collect::<Vec<_>>();
			Ok(ParseArgs::from_list(&meta_list)?)
		}
	}
}

pub fn impl_main(input: TokenStream) -> Result<TokenStream, syn::Error> {
	let span = input.span();
	let Main {
		options,
		entry,
		braced,
		items,
	} = syn::parse2(input)?;

	let crate_path = &options.crate_path;

	let service_type = quote!(#crate_path::service::Service::<#entry>);

	let global_ident = Ident::new("global", Span::mixed_site());
	let ctx_handle_ident = Ident::new("ctx_handle", Span::mixed_site());
	let services_vec_ident = Ident::new("services_vec", Span::mixed_site());
	let runtime_ident = Ident::new("runtime", Span::mixed_site());
	let config_ident = Ident::new("config", Span::mixed_site());
	let shared_global_ident = Ident::new("shared_global", Span::mixed_site());

	let handle_type =
		quote!(#crate_path::service::NamedFuture<#crate_path::prelude::tokio::task::JoinHandle<anyhow::Result<()>>>);

	let services = items.iter().filter(|item| item.item_kind == ItemKind::Service).map(|item| {
		let expr = &item.expr;
		let cfg_attrs = &item.cfg_attrs;

		let expr = quote_spanned!(Span::mixed_site().located_at(expr.span()) => #expr);

		let stringify_expr = quote! { #expr }.to_string();

		quote_spanned! { expr.span() =>
			#(#cfg_attrs)*
			{
				#[doc(hidden)]
				pub async fn spawn_service(
					svc: impl #service_type,
					global: &::std::sync::Arc<#entry>,
					ctx_handle: &#crate_path::prelude::scuffle_context::Handler,
					name: &'static str,
				) -> anyhow::Result<Option<#crate_path::service::NamedFuture<#crate_path::prelude::tokio::task::JoinHandle<anyhow::Result<()>>>>> {
					let name = #service_type::name(&svc).unwrap_or_else(|| name);
					if #crate_path::prelude::anyhow::Context::context(#service_type::enabled(&svc, &global).await, name)? {
						Ok(Some(#crate_path::service::NamedFuture::new(
							name,
							#crate_path::prelude::tokio::spawn(#service_type::run(svc, global.clone(), ctx_handle.context())),
						)))
					} else {
						Ok(None)
					}
				}

				let res = spawn_service(#expr, &#global_ident, &#ctx_handle_ident, #stringify_expr).await;

				if let Some(spawned) = res? {
					#services_vec_ident.push(spawned);
				}
			}
		}
	});

	let entry_as_global = quote_spanned! { entry.span() =>
		<#entry as #crate_path::global::Global>
	};

	let boilerplate = quote_spanned! { Span::mixed_site() =>
		#crate_path::prelude::anyhow::Context::context(#entry_as_global::pre_init(), "pre_init")?;

		let #runtime_ident = #entry_as_global::tokio_runtime();

		let #config_ident = #crate_path::prelude::anyhow::Context::context(
			#runtime_ident.block_on(
				<#entry_as_global::Config as #crate_path::config::ConfigParser>::parse()
			),
			"config parse",
		)?;

		let #ctx_handle_ident = #crate_path::prelude::scuffle_context::Handler::global();

		let mut #shared_global_ident = ::core::option::Option::None;
		let mut #services_vec_ident = ::std::vec::Vec::<#handle_type>::new();
	};

	Ok(quote! {
		#[automatically_derived]
		fn main() -> #crate_path::prelude::anyhow::Result<()> {
			#[doc(hidden)]
			pub const fn impl_global<G: #crate_path::global::Global>() {}
			const _: () = impl_global::<#entry>();

			#boilerplate

			let result = #runtime_ident.block_on(async {
				let #global_ident = #entry_as_global::init(#config_ident).await?;

				#shared_global_ident = ::core::option::Option::Some(#global_ident.clone());

				#(#services)*

				macro_rules! handle_service_exit {
					($remaining:ident) => {{
						let ((name, result), _, remaining) = #crate_path::prelude::futures::future::select_all($remaining).await;

						let result = #crate_path::prelude::anyhow::Context::context(#crate_path::prelude::anyhow::Context::context(result, name)?, name);

						#entry_as_global::on_service_exit(&#global_ident, name, result).await?;

						remaining
					}};
				}

				let mut remaining = handle_service_exit!(#services_vec_ident);

				while !remaining.is_empty() {
					remaining = handle_service_exit!(remaining);
				}

				#crate_path::prelude::anyhow::Ok(())
			});

			let ::core::option::Option::Some(global) = #shared_global_ident else {
				return result;
			};

			#runtime_ident.block_on(#entry_as_global::on_exit(&global, result))
		}
	})
}
