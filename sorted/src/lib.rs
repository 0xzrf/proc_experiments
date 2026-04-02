use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::ToTokens;
use std::any::Any;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::token::Comma;
use syn::{parse_macro_input, Error, Item, Variant};

#[proc_macro_attribute]
pub fn sorted(args: TokenStream, input: TokenStream) -> TokenStream {
    let _ = args;
    let input = parse_macro_input!(input as Item);

    match handle_sorted(input, args) {
        Ok(return_tokens) => return_tokens,
        Err((e, span)) => Error::new(span, e).to_compile_error().into(),
    }
}

fn handle_sorted(input: Item, _args: TokenStream) -> Result<TokenStream, (String, Span)> {
    println!("input type id: {:#?}", input.type_id());
    match &input {
        Item::Enum(input) => {
            are_variants_lexicographically_ordered(&input.variants)?;
        }
        _ => {
            return Err((
                "expected enum or match expression".to_string(),
                input.span(),
            ))
        }
    }

    Ok(input.to_token_stream().into())
}

fn are_variants_lexicographically_ordered(
    variants: &Punctuated<Variant, Comma>,
) -> Result<(), (String, Span)> {
    if variants.is_empty() {
        return Ok(());
    }

    let mut prev_variant = variants[0].ident.to_string();

    for variant in variants.iter().skip(1) {
        let current_variant = variant.ident.to_string();
        if prev_variant > current_variant {
            return Err((
                format!("{current_variant} should sort before {prev_variant}",),
                variant.span(),
            ));
        }
        prev_variant = current_variant;
    }
    Ok(())
}
