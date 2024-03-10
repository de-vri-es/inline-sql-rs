use proc_macro2::{TokenStream, TokenTree, Ident, Span};

#[derive(Default)]
pub struct Arguments {
	pub client: Option<syn::Expr>,
	pub map_row: Option<syn::Expr>,
	pub map_err: Option<syn::Expr>,
}

impl Arguments {
	pub fn new() -> Self {
		Default::default()
	}

	pub fn parse_params(&mut self, errors: &mut Vec<syn::Error>, tokens: TokenStream, backup_error_span: Option<Span>) {
		for arg in split_args(errors, tokens, backup_error_span) {
			if arg.ident == "client" {
				set_once(&mut self.client, arg, errors);
			} else if arg.ident == "map_row" {
				set_once(&mut self.map_row, arg, errors);
			} else if arg.ident == "map_err" {
				set_once(&mut self.map_err, arg, errors);
			} else {
				errors.push(syn::Error::new_spanned(&arg.ident, "#[inline_sql]: unrecognized argument, expected one of `client`, `map_row` or `map_err`"));
			}
		}
	}
}

fn set_once<T: syn::parse::Parse>(store_at: &mut Option<T>, arg: Arg, errors: &mut Vec<syn::Error>) {
	if store_at.is_some() {
		errors.push(syn::Error::new_spanned(&arg.ident, format!("[inline_sql]: duplicate {} argument", arg.ident)));
		return;
	}
	match syn::parse2(arg.value) {
		Err(e) => errors.push(e),
		Ok(value) => *store_at = Some(value),
	}
}

struct Arg {
	ident: proc_macro2::Ident,
	value: proc_macro2::TokenStream,
}

fn split_args(errors: &mut Vec<syn::Error>, tokens: TokenStream, backup_error_span: Option<Span>) -> Vec<Arg> {
	if tokens.is_empty() {
		return vec![];
	}
	let mut output = Vec::new();
	let mut current = TokenStream::new();
	for token in tokens {
		match token {
			proc_macro2::TokenTree::Punct(punct) if punct.as_char() == ',' => {
				match Arg::parse(std::mem::take(&mut current), Some(punct.span())) {
					Ok(arg) => output.push(arg),
					Err(e) => errors.push(e),
				}
			},
			other => {
				current.extend([other])
			}
		}
	}

	if !current.is_empty() {
		match Arg::parse(current, backup_error_span) {
			Ok(arg) => output.push(arg),
			Err(e) => errors.push(e),
		}
	}
	output
}

impl Arg {
	fn parse(tokens: TokenStream, backup_error_span: Option<Span>) -> Result<Arg, syn::Error> {
		let mut tokens = tokens.into_iter();
		let ident = expect_identifier(tokens.next(), backup_error_span.unwrap_or(Span::call_site()))?;
		let eq = expect_punct(tokens.next(), '=', backup_error_span.unwrap_or(ident.span()))?;
		let value: TokenStream = tokens.collect();
		if value.is_empty() {
			Err(syn::Error::new(backup_error_span.unwrap_or(eq.span()), "expected a value"))
		} else {
			Ok(Arg {
				ident,
				value,
			})
		}
	}
}

fn expect_identifier(token: Option<TokenTree>, backup_error_span: Span) -> Result<Ident, syn::Error> {
	match token {
		Some(TokenTree::Ident(x)) => Ok(x),
		other => {
			let span = other.map(|x| x.span()).unwrap_or(backup_error_span);
			Err(syn::Error::new(span, "#[inline_sql]: expected identifier"))
		},
	}
}

fn expect_punct(token: Option<TokenTree>, punct: char, backup_error_span: Span) -> Result<proc_macro2::Punct, syn::Error> {
	match token {
		Some(TokenTree::Punct(x)) if x.as_char() == punct => Ok(x),
		other => {
			let span = other.map(|x| x.span()).unwrap_or(backup_error_span);
			Err(syn::Error::new(span, "#[inline_sql]: expected {punct}"))
		},
	}
}
