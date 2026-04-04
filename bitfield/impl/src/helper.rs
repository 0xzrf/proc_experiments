use proc_macro::TokenStream;
use proc_macro2::Span;
use syn::punctuated::Punctuated;
use syn::token::Comma;
use syn::Field;
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

pub fn get_return_type_for_bit_size(bit_size: u8) -> Type {
    if (1..=8).contains(&bit_size) {
        syn::parse_str::<Type>("u8").unwrap()
    } else if (9..=16).contains(&bit_size) {
        syn::parse_str::<Type>("u16").unwrap()
    } else if (17..=32).contains(&bit_size) {
        syn::parse_str::<Type>("u32").unwrap()
    } else if (33..=64).contains(&bit_size) {
        syn::parse_str::<Type>("u64").unwrap()
    } else {
        panic!("Invalid bit size: {}. Only 1..=64 supported.", bit_size);
    }
}

pub fn get_struct_size_in_bits(fields: &Punctuated<Field, Comma>) -> BitFieldResult<usize> {
    let mut struct_size = 0usize;
    for field in fields {
        let field_type = &field.ty;

        struct_size += bits_from_field_type(field_type)?;
    }

    Ok(struct_size)
}
