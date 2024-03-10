use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;

use crate::input::{Arguments, SqlFunction, QueryType, Query, QueryMacro};
use crate::util::{return_type_ok_span, return_type_err_span};

pub fn expand_sql_function(errors: &mut Vec<syn::Error>, function: SqlFunction, args: Arguments) -> TokenStream {
	let SqlFunction {
		attributes,
		visibility,
		signature,
		brace_token,
		body,
	} = function;

	let Arguments {
		client,
		map_row,
		map_err,
	} = args;

	let query = match syn::parse2::<QueryMacro>(body) {
		Ok(x) => x.query,
		Err(e) => {
			errors.push(e);
			Query {
				query: String::new(),
				placeholders: Vec::new(),
			}
		},
	};
	let Query {
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

	let client = client.unwrap_or_else(|| syn::parse_quote!(client));

	let handle_err = match map_err {
		Some(map_err) => quote_spanned!(map_err.span() => {
			result.map_err(#map_err)?
		}),
		None => quote_spanned!(return_type_err_span(&signature) => {
			#[allow(clippy::useless_conversion)]
			match result.map_err(::core::convert::From::from) {
				Ok(x) => x,
				Err(e) => return Err(e),
			}
		}),
	};
	let map_elem = |typ| match map_row {
		Some(map_elem) => quote_spanned!(map_elem.span() => {
			let elem = ::inline_sql::macro_export__::convert_row(#map_elem, row);
			match elem {
				Ok(x) => x,
				Err(e) => {
					return Err(e)
				},
			}
		}),
		None => quote_spanned!(return_type_ok_span(&signature) => {
			#[allow(clippy::useless_conversion)]
			{
				let result = <#typ as ::core::convert::TryFrom<::tokio_postgres::Row>>::try_from(row);
				#handle_err
			}
		}),
	};

	let mut params = TokenStream::new();
	for placeholder in &placeholders {
		params.extend(quote_spanned!(
			placeholder.span() => &#placeholder as &(dyn ::tokio_postgres::types::ToSql + ::core::marker::Sync),
		));
	}
	let params = quote!(&[#params]);

	let body = match query_type.unwrap_or(QueryType::Execute) {
		QueryType::Execute => quote! {
			let params: &[&(dyn ::tokio_postgres::types::ToSql + ::core::marker::Sync)] = #params;
			let result: ::core::result::Result<u64, ::tokio_postgres::Error> = #client.execute(#query, params)#await_future;
			let result = #handle_err;
			Ok(())
		},
		QueryType::CountRows => quote! {
			let params: &[&(dyn ::tokio_postgres::types::ToSql + ::core::marker::Sync)] = #params;
			let result: ::core::result::Result<u64, ::tokio_postgres::Error> = #client.execute(#query, params)#await_future;
			let result = #handle_err;
			Ok(result)
		},
		QueryType::List(elem_type) => {
			let map_elem = map_elem(elem_type);
			quote! {
				let params: &[&(dyn ::tokio_postgres::types::ToSql + ::core::marker::Sync)] = #params;
				let params = params.iter().map(|x| *x as &dyn ::tokio_postgres::types::ToSql);
				let result: ::core::result::Result<::tokio_postgres::RowStream, ::tokio_postgres::Error> = #client.query_raw(#query, params)#await_future;
				let stream: ::tokio_postgres::RowStream = #handle_err;
				let mut stream = ::core::pin::pin!(stream);
				let mut output = ::std::vec::Vec::<#elem_type>::new();
				while let ::core::option::Option::Some(result) = stream.next()#await_future {
					let row = #handle_err;
					let elem = #map_elem;
					output.push(elem);
				}
				::core::result::Result::Ok(output)
			}
		},
		QueryType::Stream => quote! {
			let params: &[&(dyn ::tokio_postgres::types::ToSql + ::core::marker::Sync)] = #params;
			let params = params.iter().map(|x| *x as &dyn ::tokio_postgres::types::ToSql);
			let result: ::core::result::Result<::tokio_postgres::RowStream, ::tokio_postgres::Error> = #client.query_raw(#query, params)#await_future;
			::core::result::Result::Ok(#handle_err)
		},
		QueryType::Optional(elem_type) => {
			let map_elem = map_elem(elem_type);
			quote! {
				let params: &[&(dyn ::tokio_postgres::types::ToSql + ::core::marker::Sync)] = #params;
				let result: ::core::result::Result<::core::option::Option<::tokio_postgres::Row>, ::tokio_postgres::Error> = #client.query_opt(#query, params)#await_future;
				match #handle_err {
					::core::option::Option::None => ::core::result::Result::Ok(::core::option::Option::None),
					::core::option::Option::Some(row) => {
						let elem = #map_elem;
						::core::result::Result::Ok(::core::option::Option::Some(elem))
					},
				}
			}
		},
		QueryType::One(elem_type) => {
			let map_elem = map_elem(elem_type);
			quote! {
				let params: &[&(dyn ::tokio_postgres::types::ToSql + ::core::marker::Sync)] = #params;
				let result: ::core::result::Result<::tokio_postgres::Row, ::tokio_postgres::Error> = #client.query_one(#query, params)#await_future;
				let row = #handle_err;
				#map_elem
			}
		},
	};

	quote! {
		#(#attributes)*
		#visibility #signature {
			#[allow(unused_imports)]
			use ::inline_sql::macro_export__::prelude::*;
			#body
		}
	}
}
