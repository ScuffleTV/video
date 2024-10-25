use std::str::FromStr;

use convert_case::{Case, Casing};
use syn::punctuated::Punctuated;
use syn::Meta;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenameAll {
	Lower,
	Upper,
	Pascal,
	Camel,
	Snake,
	ScreamingSnake,
	Kebab,
	ScreamingKebab,
}

impl FromStr for RenameAll {
	type Err = ();

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s {
			"lowercase" => Ok(Self::Lower),
			"UPPERCASE" => Ok(Self::Upper),
			"PascalCase" => Ok(Self::Pascal),
			"camelCase" => Ok(Self::Camel),
			"snake_case" => Ok(Self::Snake),
			"SCREAMING_SNAKE_CASE" => Ok(Self::ScreamingSnake),
			"kebab-case" => Ok(Self::Kebab),
			"SCREAMING-KEBAB-CASE" => Ok(Self::ScreamingKebab),
			_ => Err(()),
		}
	}
}

impl RenameAll {
	/// #[serde(rename_all = "name")] or #[serde(rename_all(serialize = "name", deserialize = "name"))]
	pub fn parse(attr: &[syn::Attribute]) -> syn::Result<Option<RenameAll>> {
		parse_serde_attrs(attr, None, |state, meta| match &meta {
			Meta::NameValue(meta) if meta.path.is_ident("rename_all") => {
				if let syn::Expr::Lit(syn::ExprLit {
					lit: syn::Lit::Str(lit), ..
				}) = &meta.value
				{
					*state = Some(
						lit.value()
							.parse()
							.map_err(|_| syn::Error::new_spanned(lit, "invalid rename_all value")),
					);
				}
			}
			_ => {}
		})?
		.transpose()
	}

	pub fn apply(&self, name: &str) -> String {
		let case = match self {
			Self::Lower => Case::Lower,
			Self::Upper => Case::Upper,
			Self::Pascal => Case::Pascal,
			Self::Camel => Case::Camel,
			Self::Snake => Case::Snake,
			Self::ScreamingSnake => Case::ScreamingSnake,
			Self::Kebab => Case::Kebab,
			Self::ScreamingKebab => Case::UpperKebab,
		};

		name.to_case(case)
	}
}

#[derive(Debug, Clone)]
pub struct Name {
	pub serialize: String,
	// pub deserialize: String,
}

impl Name {
	/// #[serde(rename = "name")] or #[serde(rename(serialize = "name", deserialize = "name"))]
	pub fn parse(attr: &[syn::Attribute], name: &str, rename: Option<RenameAll>) -> syn::Result<Self> {
		parse_serde_attrs(attr, None, |state, meta| match &meta {
			Meta::NameValue(meta) if meta.path.is_ident("rename") => {
				if let syn::Expr::Lit(syn::ExprLit {
					lit: syn::Lit::Str(lit), ..
				}) = &meta.value
				{
					*state = Some(Name {
						serialize: lit.value(),
						// deserialize: lit.value(),
					})
				}
			}
			Meta::List(meta) if meta.path.is_ident("rename") => {
				let mut serialize = name.to_string();
				let mut deserialize = name.to_string();

				meta.parse_args_with(Punctuated::<Meta, syn::Token![,]>::parse_terminated)
					.unwrap_or_default()
					.iter()
					.for_each(|nested| match nested {
						Meta::NameValue(meta) if meta.path.is_ident("serialize") => {
							if let syn::Expr::Lit(syn::ExprLit {
								lit: syn::Lit::Str(lit), ..
							}) = &meta.value
							{
								serialize = lit.value();
							}
						}
						Meta::NameValue(meta) if meta.path.is_ident("deserialize") => {
							if let syn::Expr::Lit(syn::ExprLit {
								lit: syn::Lit::Str(lit), ..
							}) = &meta.value
							{
								deserialize = lit.value();
							}
						}
						_ => {}
					});

				*state = Some(Self {
					serialize,
					// deserialize,
				})
			}
			_ => {}
		})
		.transpose()
		.unwrap_or_else(|| {
			let name = rename.map(|rename| rename.apply(name)).unwrap_or(name.to_string());

			Ok(Self {
				serialize: name.clone(),
				// deserialize: name,
			})
		})
	}
}

/// #[serde(default = "default_fn")]
pub fn parse_default_fn(attrs: &[syn::Attribute]) -> syn::Result<Option<proc_macro2::TokenStream>> {
	parse_serde_attrs(attrs, None, |state, meta| match meta {
		Meta::NameValue(meta) if meta.path.is_ident("default") => {
			if let syn::Expr::Lit(syn::ExprLit {
				lit: syn::Lit::Str(lit), ..
			}) = &meta.value
			{
				*state = Some(lit.parse::<proc_macro2::TokenStream>().unwrap())
			}
		}
		_ => {}
	})
}

pub fn serde_flatten(attrs: &[syn::Attribute]) -> syn::Result<bool> {
	parse_serde_attrs(attrs, false, |state, meta| match meta {
		Meta::Path(meta) if meta.is_ident("flatten") => *state = true,
		_ => {}
	})
}

fn parse_serde_attrs<T>(attr: &[syn::Attribute], state: T, mut fold: impl FnMut(&mut T, Meta)) -> syn::Result<T> {
	attr.iter()
		.filter(|attr| attr.path().is_ident("serde"))
		.filter_map(|attr| match &attr.meta {
			Meta::List(meta) => Some(meta.parse_args_with(Punctuated::<Meta, syn::Token![,]>::parse_terminated)),
			_ => None,
		})
		.collect::<syn::Result<Vec<_>>>()?
		.into_iter()
		.flatten()
		.try_fold(state, |mut state, meta| {
			fold(&mut state, meta);
			Ok(state)
		})
}
