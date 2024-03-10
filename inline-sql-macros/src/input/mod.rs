mod args;
mod sql_function;
mod query;

pub use args::Arguments;
pub use sql_function::{SqlFunction, QueryType};
pub use query::{Query, QueryMacro};

pub enum Item {
	Function(SqlFunction),
}

impl syn::parse::Parse for Item {
	fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
		if SqlFunction::peek(input) {
			Ok(Self::Function(input.parse()?))
		} else {
			Err(syn::Error::new(proc_macro2::Span::call_site(), "#[inline_sql]: attribute must be placed on a function"))
		}
	}
}
