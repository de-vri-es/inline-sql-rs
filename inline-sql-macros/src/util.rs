use syn::spanned::Spanned;

pub fn type_strip_paren(typ: &syn::Type) -> &syn::Type {
	let mut typ = typ;
	loop {
		match typ {
			syn::Type::Paren(inner) => typ = &inner.elem,
			syn::Type::Group(inner) => typ = &inner.elem,
			x => return x,
		}
	}
}

pub fn type_as_path(typ: &syn::Type) -> Option<&syn::Path> {
	match type_strip_paren(typ) {
		syn::Type::Path(x) => Some(&x.path),
		_ => None,
	}
}

pub fn type_strip_result(typ: &syn::Type) -> Result<&syn::Type, syn::Error> {
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

pub fn type_result_args(typ: &syn::Type) -> Option<&syn::AngleBracketedGenericArguments> {
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

pub fn type_strip_vec(typ: &syn::Type) -> Option<&syn::Type> {
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

pub fn type_strip_option(typ: &syn::Type) -> Option<&syn::Type> {
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

pub fn type_is_row_stream(typ: &syn::Type) -> bool {
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

pub fn type_is_unit(typ: &syn::Type) -> bool {
	if let syn::Type::Tuple(tuple) = type_strip_paren(typ) {
		tuple.elems.is_empty()
	} else {
		false
	}
}

pub fn type_is_u64(typ: &syn::Type) -> bool {
	match type_as_path(typ) {
		None => false,
		Some(path) => path.is_ident("u64"),
	}
}

pub fn path_is(path: &syn::Path, components: &[&str]) -> bool {
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

pub fn path_is_one_of(path: &syn::Path, candidates: &[&[&str]]) -> bool {
	candidates.iter().any(|candidate| path_is(path, candidate))
}

pub fn type_span(typ: &syn::Type) -> proc_macro2::Span {
	if let Some(path) = type_as_path(typ) {
		path.segments.last().span()
	} else {
		typ.span()
	}
}

pub fn type_result_ok(typ: &syn::Type) -> Option<&syn::Type> {
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

pub fn type_result_err(typ: &syn::Type) -> Option<&syn::Type> {
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

pub fn return_type_ok_span(signature: &syn::Signature) -> proc_macro2::Span {
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

pub fn return_type_err_span(signature: &syn::Signature) -> proc_macro2::Span {
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
