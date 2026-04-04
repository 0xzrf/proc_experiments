use proc_macro::TokenStream;
use proc_macro2::Span;
use syn::{spanned::Spanned, Error, Type};

pub type BitFieldResult<T> = Result<T, (Span, &'static str)>;

pub fn generate_error(msg: &str) -> TokenStream {
    Error::new(Span::call_site(), msg).to_compile_error().into()
}

pub fn generate_error_spanned(span: Span, msg: &str) -> TokenStream {
    Error::new(span, msg).to_compile_error().into()
}

pub fn bits_from_field_type(ty: &Type) -> BitFieldResult<usize> {
    let Type::Path(ty_path) = ty else {
        return Err((Span::call_site(), "Type needs to be B1..B64"));
    };

    let bit_specifier_type = ty_path.path.segments.last().unwrap().ident.to_string();

    if !bit_specifier_type.starts_with("B") {
        return Err((
            ty_path.span(),
            "The bit specifier needs to be of type B1..B16",
        ));
    }

    let (_, bit_size) = bit_specifier_type.split_at(1);

    let Ok(bit_size) = bit_size.parse::<usize>() else {
        return Err((ty_path.span(), "Invalid Type specifier"));
    };

    if !(1..=64).contains(&bit_size) {
        return Err((ty_path.span(), "Invalid Type specifier"));
    }

    Ok(bit_size)
}
