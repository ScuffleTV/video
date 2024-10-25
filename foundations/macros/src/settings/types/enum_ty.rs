use syn::punctuated::Punctuated;
use syn::{ItemEnum, LitStr, Meta};

use super::serde::RenameAll;
use super::variant_ty::Variant;
use super::{Args, GlobalArgs};
use crate::helpers::parse_docs;

#[derive(Debug, Clone)]
pub enum EnumTagged {
	NotSpecified,
	Untagged,
	Tagged { tag: String, content: Option<String> },
}

impl EnumTagged {
	/// #[serde(tag = "tag")] or #[serde(untagged)] or #[serde(tag = "tag", content = "content")]
	fn parse(attr: &[syn::Attribute]) -> Self {
		let mut tagged = Self::NotSpecified;

		attr.iter()
			.filter(|attr| attr.path().is_ident("serde"))
			.filter_map(|attr| match &attr.meta {
				Meta::List(meta) => meta
					.parse_args_with(Punctuated::<Meta, syn::Token![,]>::parse_terminated)
					.ok(),
				_ => None,
			})
			.flatten()
			.for_each(|meta| match meta {
				Meta::NameValue(meta) if meta.path.is_ident("tag") => {
					if let syn::Expr::Lit(syn::ExprLit {
						lit: syn::Lit::Str(lit), ..
					}) = &meta.value
					{
						match &mut tagged {
							Self::Tagged { tag, .. } => *tag = lit.value(),
							_ => {
								tagged = Self::Tagged {
									tag: lit.value(),
									content: None,
								}
							}
						}
					}
				}
				Meta::NameValue(meta) if meta.path.is_ident("content") => {
					if let syn::Expr::Lit(syn::ExprLit {
						lit: syn::Lit::Str(lit), ..
					}) = &meta.value
					{
						match &mut tagged {
							Self::Tagged { tag, .. } => *tag = lit.value(),
							_ => {
								tagged = Self::Tagged {
									tag: String::new(),
									content: Some(lit.value()),
								}
							}
						}
					}
				}
				Meta::Path(path) if path.is_ident("untagged") => {
					tagged = Self::Untagged;
				}
				_ => {}
			});

		tagged
	}

	fn tag(&self) -> Option<&str> {
		match self {
			Self::Tagged { tag, .. } => Some(tag),
			_ => None,
		}
	}

	fn content(&self) -> Option<&str> {
		match self {
			Self::Tagged { content, .. } => content.as_deref(),
			_ => None,
		}
	}
}

#[derive(Debug, Clone)]
pub struct Enum {
	// pub name: Name,
	pub tagged: EnumTagged,
	pub variants: Vec<Variant>,
	pub docs: Vec<LitStr>,
	pub args: EnumArgs,
	pub item: ItemEnum,
}

impl Enum {
	pub fn new(item: ItemEnum) -> syn::Result<Self> {
		let variant_rename = RenameAll::parse(&item.attrs)?;

		let tagged = EnumTagged::parse(&item.attrs);

		Ok(Self {
			// name: Name::parse(&item.attrs, item.ident.to_string().as_str(), None)?,
			docs: parse_docs(&item.attrs),
			variants: item
				.variants
				.iter()
				.cloned()
				.map(|variant| Variant::new(variant, variant_rename))
				.collect::<syn::Result<_>>()?,
			tagged,
			args: EnumArgs::parse(&item.attrs)?,
			item,
		})
	}

	pub fn default_impl(&self) -> syn::Result<proc_macro2::TokenStream> {
		let ident = &self.item.ident;
		let (impl_generics, ty_generics, where_clause) = self.item.generics.split_for_impl();

		let default_variants = self.variants.iter().filter(|item| item.args.default).collect::<Vec<_>>();

		if default_variants.len() > 1 {
			return Err(syn::Error::new_spanned(
				&default_variants[1].item,
				"only one variant can be marked as default",
			));
		}

		let Some(default_variant) = default_variants.first() else {
			return Err(syn::Error::new_spanned(&self.item, "no default variant specified"));
		};

		let default_impl = default_variant.default_impl()?;

		Ok(quote::quote! {
			#[automatically_derived]
			impl #impl_generics Default for #ident #ty_generics #where_clause {
				#[inline]
				fn default() -> Self {
					#default_impl
				}
			}
		})
	}

	pub fn docs_impl(&self, crate_path: &syn::Path) -> syn::Result<proc_macro2::TokenStream> {
		let ident = &self.item.ident;
		let (impl_generics, ty_generics, where_clause) = self.item.generics.split_for_impl();

		let fields = self
			.variants
			.iter()
			.map(|variant| {
				let (match_token, variant_docs) = variant.docs_impl(crate_path, self.tagged.tag(), self.tagged.content())?;

				// Part of a match block
				Ok(quote::quote! {
					#ident::#match_token => {
						#variant_docs
					}
				})
			})
			.collect::<syn::Result<Vec<_>>>()?;

		let self_docs = if !self.docs.is_empty() {
			let docs = &self.docs;
			quote::quote! {
				{
					let mut parent_key = parent_key.to_vec();
					if !docs.contains_key(&parent_key) {
						docs.insert(parent_key, ::std::borrow::Cow::Borrowed(&[#(#docs),*]));
					}
				}
			}
		} else {
			quote::quote! {}
		};

		Ok(quote::quote! {
			#[automatically_derived]
			impl #impl_generics #crate_path::settings::Settings for #ident #ty_generics #where_clause {
				fn add_docs(
					&self,
					parent_key: &[::std::borrow::Cow<'static, str>],
					docs: &mut ::std::collections::HashMap<::std::vec::Vec<::std::borrow::Cow<'static, str>>, ::std::borrow::Cow<'static, [::std::borrow::Cow<'static, str>]>>,
				) {
					#self_docs
					match self {
						#(#fields)*
					}
				}
			}
		})
	}
}

#[derive(Debug, Clone, Default)]
pub struct EnumArgs {
	pub global: GlobalArgs,
}

impl Args for EnumArgs {
	fn apply_meta(&mut self, meta: &Meta) -> syn::Result<bool> {
		self.global.apply_meta(meta)
	}
}
