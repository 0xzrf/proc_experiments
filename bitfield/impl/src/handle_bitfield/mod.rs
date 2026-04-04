use crate::*;
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Fields, Item};
mod getter_and_setter;
use getter_and_setter::handle_getter_and_setter;

pub fn handle_bit_field(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as Item);

    let Item::Struct(struct_input) = input else {
        return generate_error("macro only allowed on structs atm");
    };

    let struct_name = struct_input.ident;
    let struct_vis = struct_input.vis;

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

    let accessors = match handle_getter_and_setter(named_fields.named.clone(), struct_size_in_bytes)
    {
        Ok(tokens) => tokens,
        Err((span, msg)) => return generate_error_spanned(span, msg),
    };

    quote! {
        #struct_vis struct #struct_name {
            data: [u8; #struct_size_in_bytes]
        }

        impl #struct_name {
            pub fn new() -> Self {
                Self {
                    data: [0; #struct_size_in_bytes]
                }
            }

            #accessors
        }
    }
    .into()
}
