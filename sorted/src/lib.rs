use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::ToTokens;
use std::any::Any;
use syn::{parse_macro_input, Error, Item};
#[proc_macro_attribute]
pub fn sorted(args: TokenStream, input: TokenStream) -> TokenStream {
    let _ = args;
    let input = parse_macro_input!(input as Item);

    match handle_sorted(input, args) {
        Ok(return_tokens) => return_tokens,
        Err(e) => Error::new(Span::call_site(), e).to_compile_error().into(),
    }
}

fn handle_sorted(input: Item, _args: TokenStream) -> Result<TokenStream, &'static str> {
    println!("input type id: {:#?}", input.type_id());
    if !matches!(input, Item::Enum(_)) {
        return Err("expected enum or match expression");
    }

    Ok(input.to_token_stream().into())
}
