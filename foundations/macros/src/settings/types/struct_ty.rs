use syn::{ItemStruct, LitStr, Meta};

use super::field_ty::Field;
use super::serde::RenameAll;
use super::{Args, GlobalArgs};
use crate::helpers::parse_docs;

#[derive(Debug, Clone)]
pub struct Struct {
	// pub name: Name,
	pub fields: Vec<Field>,
	pub docs: Vec<LitStr>,
	pub args: StructArgs,
	pub item: syn::ItemStruct,
}

impl Struct {
	pub fn new(item: ItemStruct) -> syn::Result<Self> {
		let field_rename = RenameAll::parse(&item.attrs)?;

		Ok(Self {
			// name: Name::parse(&item.attrs, item.ident.to_string().as_str(), None)?,
			fields: item
				.fields
				.iter()
				.cloned()
				.map(|field| Field::new(field, field_rename))
				.collect::<syn::Result<_>>()?,
			docs: parse_docs(&item.attrs),
			args: StructArgs::parse(&item.attrs)?,
			item,
		})
	}

	pub fn default_impl(&self) -> syn::Result<proc_macro2::TokenStream> {
		let ident = &self.item.ident;
		let (impl_generics, ty_generics, where_clause) = self.item.generics.split_for_impl();

		let fields = self.fields.iter().map(|field| {
			let default = field.default_impl();
			match field.item.ident.as_ref() {
				// named field
				Some(ident) => {
					quote::quote! {
						#ident: #default,
					}
				}
				// tuple struct
				None => {
					quote::quote! { #default }
				}
			}
		});

		let default_impl = match &self.item.fields {
			syn::Fields::Named(_) => {
				quote::quote! {
					Self {
						#(#fields)*
					}
				}
			}
			syn::Fields::Unnamed(_) => {
				quote::quote! {
					Self(
						#(#fields,)*
					)
				}
			}
			syn::Fields::Unit => {
				quote::quote! {
					Self
				}
			}
		};

		Ok(quote::quote! {
			#[automatically_derived]
			impl #impl_generics Default for #ident #ty_generics #where_clause {
				fn default() -> Self {
					#default_impl
				}
			}
		})
	}

	pub fn docs_impl(&self, crate_path: &syn::Path) -> syn::Result<proc_macro2::TokenStream> {
		let ident = &self.item.ident;
		let (impl_generics, ty_generics, where_clause) = self.item.generics.split_for_impl();

		let fields = self.fields.iter().map(|field| {
			let Some(name) = field.name.as_ref() else {
				return quote::quote! {};
			};

			let docs = field
				.docs
				.iter()
				.map(|doc| {
					quote::quote! { ::std::borrow::Cow::Borrowed(#doc) }
				})
				.collect::<Vec<_>>();

			let insert = if docs.is_empty() || field.flatten {
				quote::quote! {}
			} else {
				quote::quote! {
					docs.insert(parent_key, ::std::borrow::Cow::Borrowed(&[#(#docs),*]));
				}
			};

			let name = &name.serialize;
			let ident = field.item.ident.as_ref().unwrap();

			let name_push = if field.flatten {
				quote::quote! {}
			} else {
				quote::quote! {
					parent_key.push(::std::borrow::Cow::Borrowed(#name));
				}
			};

			quote::quote! {
				{
					let mut parent_key = parent_key.to_vec();
					#name_push
					(&&&&#crate_path::settings::Wrapped(&self.#ident)).add_docs(&parent_key, docs);
					#insert
				}
			}
		});

		let struct_doc = if !self.docs.is_empty() {
			let docs = self
				.docs
				.iter()
				.map(|doc| {
					quote::quote! { ::std::borrow::Cow::Borrowed(#doc) }
				})
				.collect::<Vec<_>>();

			quote::quote! {
				{
					let mut parent_key = parent_key.to_vec();
					parent_key.push(::std::borrow::Cow::Borrowed(">"));
					docs.insert(parent_key, ::std::borrow::Cow::Borrowed(&[#(#docs),*]));
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
					#struct_doc
					#(#fields)*
				}
			}
		})
	}
}

#[derive(Debug, Clone, Default)]
pub struct StructArgs {
	pub global: GlobalArgs,
}

impl Args for StructArgs {
	fn apply_meta(&mut self, meta: &Meta) -> syn::Result<bool> {
		self.global.apply_meta(meta)
	}
}
