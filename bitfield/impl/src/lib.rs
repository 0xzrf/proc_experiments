use proc_macro::TokenStream;
mod bit_fields;
use bit_fields::handle_bit_struct_definition;
mod handle_bitfield;
use handle_bitfield::handle_bit_field;
mod helper;
use helper::*;

#[proc_macro_attribute]
pub fn bitfield(_args: TokenStream, input: TokenStream) -> TokenStream {
    let _ = input;

    handle_bit_field(input)
}

#[proc_macro]
pub fn bit_fields(_input: TokenStream) -> TokenStream {
    handle_bit_struct_definition()
}
