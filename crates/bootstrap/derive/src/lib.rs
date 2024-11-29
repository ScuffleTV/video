use proc_macro::TokenStream;

mod main_impl;
// mod service_impl;

// #[proc_macro_attribute]
// pub fn service(args: TokenStream, input: TokenStream) -> TokenStream {
// 	handle_error(service_impl::impl_service(args.into(), input.into()))
// }

#[proc_macro]
pub fn main(input: TokenStream) -> TokenStream {
	handle_error(main_impl::impl_main(input.into()))
}

fn handle_error(input: Result<proc_macro2::TokenStream, syn::Error>) -> TokenStream {
	match input {
		Ok(value) => value.into(),
		Err(err) => err.to_compile_error().into(),
	}
}
