use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{quote, ToTokens};
use syn::{
    parse_macro_input,
    punctuated::Punctuated,
    spanned::Spanned,
    token::Comma,
    visit_mut::{self, VisitMut},
    Arm, Error, Item, ItemFn, Pat, Variant,
};

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

#[proc_macro_attribute]
pub fn check(_args: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ItemFn);
    handle_check(input)
}

fn handle_check(mut input: ItemFn) -> TokenStream {
    struct CheckSortedMatches {
        err: Option<(String, Span)>,
    }

    impl VisitMut for CheckSortedMatches {
        fn visit_expr_match_mut(&mut self, expr: &mut syn::ExprMatch) {
            if self.err.is_some() {
                return;
            }

            if expr.attrs.iter().any(|a| a.path().is_ident("sorted")) {
                match check_match_arm_lexicographically_ordered(&expr.arms) {
                    Ok(()) => {
                        expr.attrs.retain(|a| !a.path().is_ident("sorted"));
                    }
                    Err(e) => {
                        self.err = Some(e);
                        // Strip `#[sorted]` so expanded code is valid on stable; error is via compile_error.
                        expr.attrs.retain(|a| !a.path().is_ident("sorted"));
                        return;
                    }
                }
            }

            visit_mut::visit_expr_match_mut(self, expr);
        }
    }

    let mut visitor = CheckSortedMatches { err: None };
    visit_mut::visit_block_mut(&mut visitor, &mut input.block);

    if let Some((msg, span)) = visitor.err {
        let err_tokens = Error::new(span, msg).to_compile_error();
        return quote! {
            #err_tokens
            #input
        }
        .into();
    }

    input.to_token_stream().into()
}

/// Name used for lexicographic ordering: last path segment for struct/tuple/path patterns,
/// or a bare identifier for simple binding patterns (e.g. unit enum variants).
fn match_arm_pattern_sort_key(pat: &Pat) -> Option<(String, Span)> {
    match pat {
        Pat::Path(p) => {
            let seg = p.path.segments.last()?;
            Some((seg.ident.to_string(), seg.ident.span()))
        }
        Pat::TupleStruct(p) => {
            let seg = p.path.segments.last()?;
            Some((seg.ident.to_string(), seg.ident.span()))
        }
        Pat::Struct(p) => {
            let seg = p.path.segments.last()?;
            Some((seg.ident.to_string(), seg.ident.span()))
        }
        Pat::Ident(p) if p.subpat.is_none() => Some((p.ident.to_string(), p.ident.span())),
        _ => None,
    }
}

fn check_match_arm_lexicographically_ordered(arms: &[Arm]) -> Result<(), (String, Span)> {
    if arms.is_empty() {
        return Ok(());
    }

    let Some((mut prev_name, _)) = match_arm_pattern_sort_key(&arms[0].pat) else {
        return Err((
            "unsupported pattern in #[sorted] match".to_string(),
            arms[0].pat.span(),
        ));
    };

    for arm in arms.iter().skip(1) {
        let Some((current_name, span)) = match_arm_pattern_sort_key(&arm.pat) else {
            return Err((
                "unsupported pattern in #[sorted] match".to_string(),
                arm.pat.span(),
            ));
        };

        if prev_name > current_name {
            return Err((
                format!("{current_name} should sort before {prev_name}"),
                span,
            ));
        }
        prev_name = current_name;
    }

    Ok(())
}
