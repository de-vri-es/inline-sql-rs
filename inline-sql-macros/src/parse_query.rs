use proc_macro2::{TokenStream, TokenTree, Delimiter, Group, Span, Ident};

type TokenTreeIterator = std::iter::Peekable<<TokenStream as IntoIterator>::IntoIter>;

pub struct Query {
	pub query: String,
	pub placeholders: Vec<Ident>,
}

impl Query {
	pub fn from_tokens(tokens: TokenStream) -> Result<Self, syn::Error> {
		use std::fmt::Write;
		let mut parser = QueryParser::new(tokens);
		let mut query = String::new();
		while let Some(event) = parser.next()? {
			if !query.is_empty() {
				query.push(' ')
			}
			match event {
				Event::GroupOpen(group) => query.push(open_char(&group)?),
				Event::GroupClose(group) => query.push(close_char(&group)?),
				Event::Placeholder(pos) => write!(query, "${pos}").unwrap(),
				Event::Literal(lit) => query.push_str(&lit),
			}
		}
		Ok(Self {
			query,
			placeholders: parser.placeholders,
		})
	}
}

struct QueryParser {
	stack: Vec<(TokenTreeIterator, Option<Group>)>,
	placeholders: Vec<Ident>,
}

impl QueryParser {
	fn new(tokens: TokenStream) -> Self {
		Self {
			stack: vec![(tokens.into_iter().peekable(), None)],
			placeholders: Vec::new(),
		}
	}

	fn next(&mut self) -> Result<Option<Event>, syn::Error> {
		let (tokens, _group) = match self.stack.last_mut() {
			Some(x) => x,
			None => return Ok(None),
		};

		let tree = match tokens.next() {
			Some(tree) => tree,
			None => {
				let (_, group) = self.stack.pop().unwrap();
				match group {
					Some(group) => {
						return Ok(Some(Event::GroupClose(group)));
					},
					None => {
						if self.stack.is_empty() {
							return Ok(None);
						} else {
							return Err(syn::Error::new(Span::call_site(), "found stack entry without group that is not the root TokenStream"));
						}
					}
				}
			},
		};

		match tree {
			TokenTree::Group(group) => {
				self.stack.push((group.stream().into_iter().peekable(), Some(group.clone())));
				Ok(Some(Event::GroupOpen(group)))
			},
			TokenTree::Ident(ident) => {
				Ok(Some(Event::Literal(ident.to_string())))
			},
			TokenTree::Punct(punct) => {
				if punct.as_char() == '$' {
					let ident = take_placeholder(tokens)
						.map_err(|span| syn::Error::new(span.unwrap_or(punct.span()), "#[inline_sql]: expected placeholder name"))?;
					let pos = self.map_placeholder(ident);
					Ok(Some(Event::Placeholder(pos)))
				} else {
					let mut data = punct.to_string();
					while let Some(TokenTree::Punct(punct)) = tokens.peek() {
						if punct.as_char() == '$' {
							break;
						}
						data.push(punct.as_char());
						tokens.next();
					}
					Ok(Some(Event::Literal(data)))
				}
			},
			TokenTree::Literal(literal) => {
				Ok(Some(Event::Literal(literal.to_string())))
			}
		}
	}

	#[allow(clippy::cmp_owned)]
	fn map_placeholder(&mut self, ident: Ident) -> usize {
		let name = ident.to_string();
		if let Some(pos) = self.placeholders.iter().position(|x| x.to_string() == name) {
			pos + 1
		} else {
			let pos = self.placeholders.len();
			self.placeholders.push(ident);
			pos + 1
		}
	}
}

fn take_placeholder(tokens: &mut TokenTreeIterator) -> Result<Ident, Option<Span>> {
	match tokens.next() {
		Some(TokenTree::Ident(ident)) => Ok(ident),
		None => Err(None),
		Some(other) => Err(Some(other.span())),
	}
}

enum Event {
	GroupOpen(Group),
	GroupClose(Group),
	Placeholder(usize),
	Literal(String),
}


fn open_char(group: &Group) -> Result<char, syn::Error> {
	match group.delimiter() {
		Delimiter::None => Err(syn::Error::new(group.span(), "#[inline-sql] encountered a none-delimited group in the query")),
		Delimiter::Brace => Ok('{'),
		Delimiter::Parenthesis => Ok('('),
		Delimiter::Bracket => Ok('['),
	}
}

fn close_char(group: &Group) -> Result<char, syn::Error> {
	match group.delimiter() {
		Delimiter::None => Err(syn::Error::new(group.span(), "#[inline-sql] encountered a none-delimited group in the query")),
		Delimiter::Brace => Ok('}'),
		Delimiter::Parenthesis => Ok(')'),
		Delimiter::Bracket => Ok(']'),
	}
}
