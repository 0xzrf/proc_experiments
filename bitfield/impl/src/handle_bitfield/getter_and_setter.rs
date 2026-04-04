use proc_macro::TokenStream;
use syn::{punctuated::Punctuated, token::Comma, Field};

pub fn handle_getter_and_setter(fields: Punctuated<Field, Comma>) -> TokenStream {
    for field in &fields {}

    quote::quote! {}.into()
}
