use proc_macro2::TokenStream;
use quote::quote;
use syn::spanned::Spanned;

mod parse_query;

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
	if !params.is_empty() {
		errors.push(syn::Error::new_spanned(params, "#[inline_sql]: unsupported attribute parameter"));
	}

	let item: Item = match syn::parse2(item.clone()) {
		Ok(x) => x,
		Err(e) => {
			errors.push(e);
			return item;
		}
	};

	match item {
		Item::Function(function) => inline_sql_function(errors, function),
	}
}

fn inline_sql_function(errors: &mut Vec<syn::Error>, function: SqlFunction) -> TokenStream {
	let SqlFunction {
		attributes,
		visibility,
		signature,
		brace_token,
		body,
	} = function;

	let query = match parse_query::Query::from_tokens(body.tokens) {
		Ok(x) => x,
		Err(e) => {
			errors.push(e);
			parse_query::Query {
				query: String::new(),
				placeholders: Vec::new(),
			}
		},
	};
	let parse_query::Query {
		query,
		placeholders
	} = query;

	let _ = brace_token;

	let query_type = match &signature.output {
		syn::ReturnType::Default => {
			errors.push(syn::Error::new_spanned(&signature.ident, "#[inline_sql]: Function must return a `Result<_, _>`"));
			None
		},
		syn::ReturnType::Type(_, typ) => {
			QueryType::from_return_type(typ)
				.map_err(|e| errors.push(e))
				.ok()
		}
	};


	let await_future = if signature.asyncness.is_some() {
		Some(quote!(.await))
	} else {
		None
	};

	let handle_err = quote::quote_spanned!(return_type_err_span(&signature) => {
		#[allow(clippy::useless_conversion)]
		match result {
			Ok(x) => x,
			Err(e) => return Err(::core::convert::From::from(e)),
		}
	});
	let map_elem = |typ| quote::quote_spanned!(return_type_ok_span(&signature) => {
		#[allow(clippy::useless_conversion)]
		let result = <#typ as ::core::convert::TryFrom<::tokio_postgres::Row>>::try_from(row);
		#handle_err
	});

	let placeholders = quote! {
		&[#(&#placeholders as &(dyn ::tokio_postgres::types::ToSql + ::core::marker::Sync)),*]
	};

	let body = match query_type.unwrap_or(QueryType::Execute) {
		QueryType::Execute => quote! {
			let params: &[&(dyn ::tokio_postgres::types::ToSql + ::core::marker::Sync)] = #placeholders;
			let result = client.execute(#query, params)#await_future;
			let result = #handle_err;
			Ok(())
		},
		QueryType::CountRows => quote! {
			let params: &[&(dyn ::tokio_postgres::types::ToSql + ::core::marker::Sync)] = #placeholders;
			let result = client.execute(#query, params)#await_future;
			let result = #handle_err;
			Ok(result)
		},
		QueryType::List(elem_type) => {
			let map_elem = map_elem(elem_type);
			quote! {
				let params: &[&(dyn ::tokio_postgres::types::ToSql + ::core::marker::Sync)] = #placeholders;
				let params = params.iter().map(|x| *x as &dyn ::tokio_postgres::types::ToSql);
				let result = client.query_raw(#query, params)#await_future;
				let mut stream = ::core::pin::pin!(#handle_err);
				let mut output = ::std::vec::Vec::new();
				while let ::core::option::Option::Some(result) = stream.next()#await_future {
					let row = #handle_err;
					let elem = #map_elem;
					output.push(elem);
				}
				::core::result::Result::Ok(output)
			}
		},
		QueryType::Stream => quote! {
			let params: &[&(dyn ::tokio_postgres::types::ToSql + ::core::marker::Sync)] = #placeholders;
			let params = params.iter().map(|x| *x as &dyn ::tokio_postgres::types::ToSql);
			let result = client.query_raw(#query, params)#await_future;
			::core::result::Result::Ok(#handle_err)
		},
		QueryType::Optional(elem_type) => {
			let map_elem = map_elem(elem_type);
			quote! {
				let params: &[&(dyn ::tokio_postgres::types::ToSql + ::core::marker::Sync)] = #placeholders;
				let result = client.query_opt(#query, params)#await_future;
				match #handle_err {
					::core::option::Option::None => ::core::result::Result::Ok(::core::option::Option::None),
					::core::option::Option::Some(row) => {
						::core::result::Result::Ok(::core::option::Option::Some(#map_elem))
					},
				}
			}
		},
		QueryType::One(elem_type) => {
			let map_elem = map_elem(elem_type);
			quote! {
				let params: &[&(dyn ::tokio_postgres::types::ToSql + ::core::marker::Sync)] = #placeholders;
				let result = client.query_one(#query, params)#await_future;
				let row = #handle_err;
				#map_elem
			}
		},
	};

	quote! {
		#(#attributes)*
		#visibility #signature {
			#[allow(unused_import)]
			use ::inline_sql::macro_export__::prelude::*;
			#body
		}
	}
}

enum Item {
	Function(SqlFunction),
}

struct SqlFunction {
	pub attributes: Vec<syn::Attribute>,
	pub visibility: syn::Visibility,
	pub signature: syn::Signature,
	#[allow(unused)]
	pub brace_token: syn::token::Brace,
	pub body: Query,
}

mod token {
	syn::custom_keyword!(query);
}

struct Query {
	#[allow(unused)]
	pub query_token: token::query,
	#[allow(unused)]
	pub exclamation: syn::token::Not,
	#[allow(unused)]
	pub delimiter: syn::MacroDelimiter,
	pub tokens: TokenStream,
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

impl SqlFunction {
	#[allow(clippy::nonminimal_bool)]
	fn peek(input: syn::parse::ParseStream) -> bool {
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

impl syn::parse::Parse for Query {
	fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
		let query_token = input.parse()?;
		let exclamation = input.parse()?;
		let (delimiter, tokens) = parse_delimiter(input)?;
		Ok(Self {
			query_token,
			exclamation,
			delimiter,
			tokens,
		})
	}
}

fn parse_delimiter(input: syn::parse::ParseStream) -> Result<(syn::MacroDelimiter, TokenStream), syn::Error> {
	use proc_macro2::{Delimiter, TokenTree};
	input.step(|cursor| {
		if let Some((TokenTree::Group(g), rest)) = cursor.token_tree() {
			let span = g.delim_span();
			let delimiter = match g.delimiter() {
				Delimiter::Parenthesis => syn::MacroDelimiter::Paren(syn::token::Paren(span)),
				Delimiter::Brace => syn::MacroDelimiter::Brace(syn::token::Brace(span)),
				Delimiter::Bracket => syn::MacroDelimiter::Bracket(syn::token::Bracket(span)),
				Delimiter::None => {
					return Err(cursor.error("expected delimiter"));
				}
			};
			Ok(((delimiter, g.stream()), rest))
		} else {
			Err(cursor.error("expected delimiter"))
		}
	})
}

enum QueryType<'a> {
	Execute,
	CountRows,
	List(&'a syn::Type),
	Optional(&'a syn::Type),
	One(&'a syn::Type),
	Stream,
}

impl<'a> QueryType<'a> {
	fn from_return_type(typ: &'a syn::Type) -> Result<Self, syn::Error> {
		let typ = type_strip_result(typ)?;
		if let Some(inner) = type_strip_vec(typ) {
			Ok(Self::List(inner))
		} else if let Some(inner) = type_strip_option(typ) {
			Ok(Self::Optional(inner))
		} else if type_is_row_stream(typ) {
			Ok(Self::Stream)
		} else if type_is_unit(typ) {
			Ok(Self::Execute)
		} else if type_is_u64(typ) {
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

fn type_strip_paren(typ: &syn::Type) -> &syn::Type {
	let mut typ = typ;
	loop {
		match typ {
			syn::Type::Paren(inner) => typ = &inner.elem,
			syn::Type::Group(inner) => typ = &inner.elem,
			x => return x,
		}
	}
}

fn type_as_path(typ: &syn::Type) -> Option<&syn::Path> {
	match type_strip_paren(typ) {
		syn::Type::Path(x) => Some(&x.path),
		_ => None,
	}
}

fn type_strip_result(typ: &syn::Type) -> Result<&syn::Type, syn::Error> {
	fn short_error<S: syn::spanned::Spanned + quote::ToTokens>(spanned: S) -> syn::Error {
		syn::Error::new_spanned(spanned, "The function must return a `Result<_, _>`")
	}

	fn long_error<S: syn::spanned::Spanned + quote::ToTokens>(spanned: S, message: &'static str) -> syn::Error {
		let note = concat!(
			"Note: The function must return a `Result<_, _>`.\n",
			"If you are using a type alias, the macro can not resolve it.\n",
			"Replace the alias with the actual type name.",
		);
		syn::Error::new_spanned(spanned, format!("{message}\n\n{note}"))
	}

	let path = type_as_path(typ)
		.ok_or_else(|| short_error(typ))?;
	let segment = path.segments.last()
		.ok_or_else(|| short_error(path))?;

	if segment.ident != "Result" {
		return Err(long_error(segment, "Expected `Result<_, _>`"));
	}

	let arguments = match &segment.arguments {
		syn::PathArguments::AngleBracketed(arguments) => arguments,
		_ => return Err(long_error(segment, "Expected `Result<_, _>`")),
	};

	if arguments.args.is_empty() || arguments.args.len() > 2 {
		return Err(long_error(segment, "Expected `Result<_, _>`"));
	}

	if let syn::GenericArgument::Type(typ) = &arguments.args[0] {
		Ok(typ)
	} else {
		Err(long_error(&arguments.args[0], "Expected a type argument"))
	}
}

fn type_result_args(typ: &syn::Type) -> Option<&syn::AngleBracketedGenericArguments> {
	let path = type_as_path(typ)?;
	let segment = path.segments.last()?;
	if segment.ident != "Result" {
		return None;
	}

	match &segment.arguments {
		syn::PathArguments::AngleBracketed(arguments) => Some(arguments),
		_ => None,
	}
}

fn type_result_ok(typ: &syn::Type) -> Option<&syn::Type> {
	let args = type_result_args(typ)?;
	if args.args.is_empty() {
		return None;
	}

	if let syn::GenericArgument::Type(typ) = &args.args[0] {
		Some(typ)
	} else {
		None
	}
}

fn type_result_err(typ: &syn::Type) -> Option<&syn::Type> {
	let args = type_result_args(typ)?;
	if args.args.len() != 2 {
		return None;
	}

	if let syn::GenericArgument::Type(typ) = &args.args[1] {
		Some(typ)
	} else {
		None
	}
}

fn type_strip_vec(typ: &syn::Type) -> Option<&syn::Type> {
	let path = type_as_path(typ)?;

	let candidates = &[
		["Vec"].as_slice(),
		["std", "vec", "Vec"].as_slice(),
		["alloc", "vec", "Vec"].as_slice(),
		["", "std", "vec", "Vec"].as_slice(),
		["", "alloc", "vec", "Vec"].as_slice(),
	];

	if !path_is_one_of(path, candidates) {
		return None;
	}

	let last = path.segments.last()?;
	let arguments = match &last.arguments {
		syn::PathArguments::AngleBracketed(args) => args,
		_ => return None,
	};

	match &arguments.args[0] {
		syn::GenericArgument::Type(x) => Some(x),
		_ => None,
	}
}

fn type_strip_option(typ: &syn::Type) -> Option<&syn::Type> {
	let path = type_as_path(typ)?;

	let candidates = &[
		["Option"].as_slice(),
		["std", "option", "Option"].as_slice(),
		["core", "option", "Option"].as_slice(),
		["", "std", "option", "Option"].as_slice(),
		["", "core", "option", "Option"].as_slice(),
	];

	if !path_is_one_of(path, candidates) {
		return None;
	}

	let last = path.segments.last()?;
	let arguments = match &last.arguments {
		syn::PathArguments::AngleBracketed(args) => args,
		_ => return None,
	};

	match &arguments.args[0] {
		syn::GenericArgument::Type(x) => Some(x),
		_ => None,
	}
}

fn type_is_row_stream(typ: &syn::Type) -> bool {
	let candidates = &[
		["RowStream"].as_slice(),
		["tokio_postgres", "RowStream"].as_slice(),
		["", "tokio_postgres", "RowStream"].as_slice(),
		["RowIter"].as_slice(),
		["postgres", "RowIter"].as_slice(),
		["", "postgres", "RowIter"].as_slice(),
	];

	if let Some(path) = type_as_path(typ) {
		path_is_one_of(path, candidates)
	} else {
		false
	}
}

fn type_is_unit(typ: &syn::Type) -> bool {
	if let syn::Type::Tuple(tuple) = type_strip_paren(typ) {
		tuple.elems.is_empty()
	} else {
		false
	}
}

fn type_is_u64(typ: &syn::Type) -> bool {
	match type_as_path(typ) {
		None => false,
		Some(path) => path.is_ident("u64"),
	}
}

fn path_is(path: &syn::Path, components: &[&str]) -> bool {
	if path.segments.len() != components.len() {
		return false;
	}

	for (segment, component) in path.segments.iter().zip(components.iter()) {
		if segment.ident != component {
			return false;
		}
	}

	true
}

fn path_is_one_of(path: &syn::Path, candidates: &[&[&str]]) -> bool {
	candidates.iter().any(|candidate| path_is(path, candidate))
}

fn return_type_ok_span(signature: &syn::Signature) -> proc_macro2::Span {
	match &signature.output {
		syn::ReturnType::Default => signature.ident.span(),
		syn::ReturnType::Type(_, typ) => {
			if let Some(typ) = type_result_ok(typ) {
				type_span(typ)
			} else {
				type_span(typ)
			}
		}
	}
}

fn return_type_err_span(signature: &syn::Signature) -> proc_macro2::Span {
	match &signature.output {
		syn::ReturnType::Default => signature.ident.span(),
		syn::ReturnType::Type(_, typ) => {
			if let Some(typ) = type_result_err(typ) {
				type_span(typ)
			} else {
				type_span(typ)
			}
		}
	}
}

fn type_span(typ: &syn::Type) -> proc_macro2::Span {
	if let Some(path) = type_as_path(typ) {
		path.segments.last().span()
	} else {
		typ.span()
	}
}
