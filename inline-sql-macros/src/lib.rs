use proc_macro2::TokenStream;
use quote::quote;

mod input;
mod expand;
mod util;

#[proc_macro_attribute]
pub fn inline_sql(params: proc_macro::TokenStream, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
	let mut errors = Vec::new();
	let output = inline_sql_impl(&mut errors, params.into(), item.into());

	let errors = errors.iter().map(|x| x.to_compile_error());
	quote! {
		#(#errors)*
		#output
	}.into()
}

fn inline_sql_impl(errors: &mut Vec<syn::Error>, params: TokenStream, item: TokenStream) -> TokenStream {
	let mut args = input::Arguments::new();
	args.parse_params(errors, params, None);

	let item: input::Item = match syn::parse2(item.clone()) {
		Ok(x) => x,
		Err(e) => {
			errors.push(e);
			return item;
		}
	};

	match item {
		input::Item::Function(function) => expand::expand_sql_function(errors, function, args),
	}
}
