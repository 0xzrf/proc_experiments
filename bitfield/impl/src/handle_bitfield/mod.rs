use crate::*;
use proc_macro::TokenStream;
use quote::quote;
use syn::punctuated::Punctuated;
use syn::token::Comma;
use syn::{parse_macro_input, Field, Fields, Item};

pub fn handle_bit_field(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as Item);

    let Item::Struct(struct_input) = input else {
        return generate_error("macro only allowed on structs atm");
    };

    let struct_name = struct_input.ident;

    let Fields::Named(named_fields) = struct_input.fields else {
        return generate_error("only named fields allowed for this attribute");
    };

    let struct_size_size_in_bits = match get_struct_size_in_bits(&named_fields.named) {
        Ok(size) => size,
        Err((span, msg)) => return generate_error_spanned(span, msg),
    };

    if struct_size_size_in_bits % 8 != 0 {
        return generate_error("The bit size of the struct should be a multiple of 8");
    }

    let struct_size_in_bytes = struct_size_size_in_bits / 8;

    quote! {
        struct #struct_name {
            data: [u8; #struct_size_in_bytes]
        }
    }
    .into()
}

fn get_struct_size_in_bits(fields: &Punctuated<Field, Comma>) -> BitFieldResult<usize> {
    let mut struct_size = 0usize;
    for field in fields {
        let field_type = &field.ty;

        struct_size += bits_from_field_type(field_type)?;
    }

    Ok(struct_size)
}
