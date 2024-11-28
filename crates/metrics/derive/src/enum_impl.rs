use darling::ast::NestedMeta;
use darling::FromMeta;
use proc_macro2::TokenStream;

#[derive(Debug, darling::FromMeta)]
struct VariantAttr {
	rename: Option<syn::LitStr>,
}

#[derive(Debug, darling::FromMeta)]
struct EnumAttr {
	crate_path: Option<syn::Path>,
}

pub fn metric_enum_impl(input: TokenStream) -> syn::Result<TokenStream> {
	let input = syn::parse2::<syn::ItemEnum>(input)?;
	let enum_ident = &input.ident;

	// We only support C-style enums
	let branches = input
		.variants
		.iter()
		.map(|variant| {
			let ident = &variant.ident;
			if matches!(variant.fields, syn::Fields::Named(_) | syn::Fields::Unnamed(_)) {
				return Err(syn::Error::new_spanned(ident, "only unit enums are supported"));
			}

			// #[metrics(rename = "...")]
			let mut meta = Vec::new();
			for attr in &variant.attrs {
				if attr.path().is_ident("metrics") {
					match &attr.meta {
						syn::Meta::List(syn::MetaList { tokens, .. }) => {
							meta.extend(NestedMeta::parse_meta_list(tokens.clone())?);
						}
						_ => return Err(syn::Error::new_spanned(attr, "expected list")),
					}
				}
			}

			let options = VariantAttr::from_list(&meta)?;

			let name = options.rename.map(|lit| lit.value()).unwrap_or_else(|| ident.to_string());

			Ok(quote::quote! {
				#enum_ident::#ident => #name,
			})
		})
		.collect::<syn::Result<Vec<_>>>()?;

	let mut meta = Vec::new();
	for attr in &input.attrs {
		if attr.path().is_ident("metrics") {
			match &attr.meta {
				syn::Meta::List(syn::MetaList { tokens, .. }) => {
					meta.extend(NestedMeta::parse_meta_list(tokens.clone())?);
				}
				_ => return Err(syn::Error::new_spanned(attr, "expected list")),
			}
		}
	}

	let options = EnumAttr::from_list(&meta)?;

	let crate_path = options.crate_path.unwrap_or_else(|| syn::parse_quote!(::scuffle_metrics));

	Ok(quote::quote! {
		impl ::core::convert::From<#enum_ident> for #crate_path::opentelemetry::Value {
			fn from(value: #enum_ident) -> Self {
				let value = match value {
					#(#branches)*
				};

				#crate_path::opentelemetry::Value::String(value.into())
			}
		}
	})
}
