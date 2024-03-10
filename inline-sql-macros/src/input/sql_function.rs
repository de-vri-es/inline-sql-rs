use proc_macro2::TokenStream;

use crate::util;

pub struct SqlFunction {
	pub attributes: Vec<syn::Attribute>,
	pub visibility: syn::Visibility,
	pub signature: syn::Signature,
	#[allow(unused)]
	pub brace_token: syn::token::Brace,
	pub body: TokenStream,
}

pub enum QueryType<'a> {
	Execute,
	CountRows,
	List(&'a syn::Type),
	Optional(&'a syn::Type),
	One(&'a syn::Type),
	Stream,
}

impl SqlFunction {
	#[allow(clippy::nonminimal_bool)]
	pub fn peek(input: syn::parse::ParseStream) -> bool {
		if input.peek(syn::token::Fn) {
			return true;
		}
		let fork = input.fork();
		true
			&& fork.parse::<Option<syn::token::Const>>().is_ok()
			&& fork.parse::<Option<syn::token::Async>>().is_ok()
			&& fork.parse::<Option<syn::token::Unsafe>>().is_ok()
			&& fork.parse::<Option<syn::Abi>>().is_ok()
			&& fork.peek(syn::token::Fn)
	}
}

impl syn::parse::Parse for SqlFunction {
	fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
		let body;
		Ok(Self {
			attributes: syn::Attribute::parse_outer(input)?,
			visibility: input.parse()?,
			signature: input.parse()?,
			brace_token: syn::braced!(body in input),
			body: body.parse()?,
		})
	}
}

impl<'a> QueryType<'a> {
	pub fn from_return_type(typ: &'a syn::Type) -> Result<Self, syn::Error> {
		let typ = util::type_strip_result(typ)?;
		if let Some(inner) = util::type_strip_vec(typ) {
			Ok(Self::List(inner))
		} else if let Some(inner) = util::type_strip_option(typ) {
			Ok(Self::Optional(inner))
		} else if util::type_is_row_stream(typ) {
			Ok(Self::Stream)
		} else if util::type_is_unit(typ) {
			Ok(Self::Execute)
		} else if util::type_is_u64(typ) {
			Ok(Self::CountRows)
		} else {
			Err(syn::Error::new_spanned(typ, concat!(
				"#[inline_sql]: Expected `()`, `u64`, `Vec<_>`, `Option<_>`, `RowStream` or `RowIter`\n\n",
				"Note: the macro uses the return type to determine what kind and how many results the query returns.\n",
				"Please make sure the query returns one of the supported types.\n",
				"If you are using a type alias, the macro can not resolve it.\n",
				"Replace the alias with the actual type name.",
			)))
		}
	}
}
