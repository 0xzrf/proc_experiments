//! Attribute macros that enforce **lexicographic** ordering of enum variant names and of
//! `match` arms (when checked via [`check`]).
//!
//! # [`sorted`]: enum definitions
//!
//! Attach [`sorted`] to an `enum` item. The macro parses the item and checks that variant
//! identifiers appear in **non-decreasing** lexicographic order (by UTF-8 string comparison of
//! the variant name only). Associated data on variants is ignored for ordering; only the
//! variant’s identifier matters.
//!
//! # [`check`]: `#[sorted]` on `match` inside functions
//!
//! You cannot rely on the compiler accepting `#[sorted]` on a `match` expression in all contexts,
//! so this crate provides [`check`]: put `#[sorted::check]` on the **function** (or method)
//! that contains one or more `match` expressions. The macro walks the function body, finds each
//! `match` that carries an inner `#[sorted]` attribute, validates arm order, then **removes** that
//! inner `#[sorted]` attribute from the expanded code so the rest compiles as ordinary Rust.
//!
//! Match-arm ordering uses a string “sort key” derived from each arm’s pattern (path segments
//! joined with `::`, bare identifiers, or `"_"` for wildcards). Unsupported patterns fail with a
//! targeted error.
//!
//! # Further reading
//!
//! The integration tests under `sorted/tests/` mirror the behavior described here (e.g.
//! `01-parse-enum.rs` through `08-underscore.rs`).

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

/// Enforces lexicographic order of **variant names** on an `enum` definition.
///
/// # Accepted input
///
/// The attribute must be placed on an `enum` item. The macro returns the same item unchanged when
/// the check passes (pass-through expansion).
///
/// # Ordering rule
///
/// Let `name(v)` be the variant’s identifier as a string. For consecutive variants `a`, `b` in
/// source order, the macro requires `name(a) <= name(b)` using standard `str` ordering.
///
/// # Examples that compile
///
/// ```rust
/// # use sorted::sorted;
/// #[sorted]
/// pub enum Conference {
///     RustBeltRust,
///     RustConf,
///     RustFest,
///     RustLatam,
///     RustRush,
/// }
/// ```
///
/// Variants with data are ordered **only** by the variant name (`Fmt`, `Io`, `Utf8`, …), not by
/// the types inside the tuple:
///
/// ```rust
/// # use sorted::sorted;
/// # use std::fmt;
/// # use std::io;
/// #[sorted]
/// pub enum Error {
///     Fmt(fmt::Error),
///     Io(io::Error),
/// }
/// ```
///
/// An empty variant list is allowed.
///
/// ```rust
/// # use sorted::sorted;
/// #[sorted]
/// pub enum Empty {}
/// ```
///
/// # Errors
///
/// ## Wrong item kind
///
/// If `#[sorted]` is applied to anything that is not an `enum` (for example a `struct`), expansion
/// fails with an error at the item’s span:
///
/// ```text
/// error: expected enum or match expression
/// ```
///
/// (The message is historical; this implementation only validates `enum` items.)
///
/// ## Out-of-order variant names
///
/// The error points at the **first variant that breaks** non-decreasing order and names which
/// earlier variant it should appear before:
///
/// ```text
/// error: SomethingFailed should sort before ThisFailed
/// ```
///
/// Example (from the workshop tests): `ThatFailed`, `ThisFailed`, `SomethingFailed`, … must be
/// sorted by name; putting `SomethingFailed` after `ThisFailed` triggers the error on
/// `SomethingFailed`.
///
/// With tuple or struct variants, the same rule applies to identifiers only, e.g. `Dyn` before
/// `Var`:
///
/// ```text
/// error: Dyn should sort before Var
/// ```
///
/// # Arguments
///
/// The attribute accepts no meaningful arguments; any tokens are ignored.
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

/// Scans a function body for `match` expressions annotated with `#[sorted]` and enforces the same
/// lexicographic discipline on **match arms** as [`sorted`] does on enum variants.
///
/// # Why this macro exists
///
/// Place `#[sorted::check]` on the surrounding `fn` or inherent method. It searches the body for
/// `match` expressions that still have a `#[sorted]` attribute, validates arm order, then strips
/// `#[sorted]` off those `match` nodes so the compiler never has to treat `#[sorted]` as an
/// unknown attribute on an expression in contexts where that would be rejected.
///
/// # Supported match patterns (sort keys)
///
/// For each arm, a string key is computed for comparison:
///
/// - **`Pat::Path`**: all path segments joined with `::` (e.g. `Error::Io` → `"Error::Io"`).
/// - **`Pat::TupleStruct` / `Pat::Struct`**: same full path string as for `Pat::Path`.
/// - **`Pat::Ident`** (no sub-pattern): the identifier alone (typical for `use Enum::*` style).
/// - **`Pat::Wild` (`_`)**: the key is `"_"`, which sorts after normal identifiers; see wildcard
///   rules below.
///
/// Any other pattern is **unsupported** and fails the check.
///
/// # Ordering rule
///
/// Non-decreasing order of those string keys, compared with `str` ordering, for arms in source
/// order.
///
/// # Wildcard (`_`) arms
///
/// If `_` is used, it must be the **last** arm when there are multiple arms. Otherwise you get a
/// “wildcard cannot be present before other arms” style error (see implementation for exact
/// wording). A single-arm match containing only `_` is allowed.
///
/// # Examples that compile
///
/// Enum plus a checked `match` using imported variant names (`Fmt`, `Io`) in sorted order:
///
/// ```rust
/// mod example {
///     use sorted::sorted;
///     use std::fmt::{self, Display};
///     use std::io;
///
///     #[sorted]
///     pub enum Error {
///         Fmt(fmt::Error),
///         Io(io::Error),
///     }
///
///     impl Display for Error {
///         #[sorted::check]
///         fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
///             use self::Error::*;
///
///             #[sorted]
///             match self {
///                 Fmt(e) => write!(f, "{}", e),
///                 Io(e) => write!(f, "{}", e),
///             }
///         }
///     }
/// }
/// ```
///
/// Qualified paths are ordered by the full string (`Error::Fmt` before `Error::Io`):
///
/// ```rust
/// # use sorted::sorted;
/// # use std::fmt::{self, Display};
/// # use std::io;
/// #[sorted]
/// pub enum Error {
///     Fmt(fmt::Error),
///     Io(io::Error),
/// }
///
/// impl Display for Error {
///     #[sorted::check]
///     fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
///         #[sorted]
///         match self {
///             Error::Fmt(e) => write!(f, "{}", e),
///             Error::Io(e) => write!(f, "{}", e),
///         }
///     }
/// }
/// ```
///
/// Wildcard last (sorted keys: `RustFest`, `RustLatam`, `_`):
///
/// ```rust
/// mod example {
///     use sorted::sorted;
///
///     #[sorted]
///     pub enum Conference {
///         RustBeltRust,
///         RustConf,
///         RustFest,
///         RustLatam,
///         RustRush,
///     }
///
///     impl Conference {
///         #[sorted::check]
///         pub fn region(&self) -> &str {
///             use self::Conference::*;
///             #[sorted]
///             match self {
///                 RustFest => "Europe",
///                 RustLatam => "Latin America",
///                 _ => "elsewhere",
///             }
///         }
///     }
/// }
/// ```
///
/// # Errors
///
/// ## Out-of-order arms
///
/// Typical message when tuple struct / ident patterns are out of order:
///
/// ```text
/// error: Fmt should sort before Io
/// ```
///
/// When paths are compared:
///
/// ```text
/// error: Error::Fmt should sort before Error::Io
/// ```
///
/// ## Unsupported patterns
///
/// If the **first** arm uses a pattern that does not yield a sort key (e.g. slice patterns like
/// `[]`), the error is:
///
/// ```text
/// error: unsupported by #[sorted]
/// ```
///
/// If a later arm is unsupported:
///
/// ```text
/// error: unsupported pattern in #[sorted] match
/// ```
///
/// ## Wildcard not last
///
/// Using `_` before other arms is rejected; the exact message includes index and arm count for
/// non-first wildcards (see the implementation of the match-arm walk).
///
/// # Interaction with errors
///
/// When validation fails, the macro emits a `compile_error!`-style token stream **and** still
/// emits the original function tokens so you see the macro error at the right span while the
/// surrounding code remains visible in diagnostics.
///
/// # Arguments
///
/// The attribute accepts no meaningful arguments; any tokens are ignored.
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
            let seg = p
                .path
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect::<Vec<_>>()
                .join("::");
            Some((seg, p.path.segments.span()))
        }
        Pat::TupleStruct(p) => {
            let seg = p
                .path
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect::<Vec<String>>()
                .join("::");

            Some((seg, p.path.segments.span()))
        }
        Pat::Struct(p) => {
            let seg = p
                .path
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect::<Vec<_>>()
                .join("::");
            Some((seg, p.path.segments.span()))
        }
        Pat::Ident(p) if p.subpat.is_none() => Some((p.ident.to_string(), p.ident.span())),
        Pat::Wild(wild) => Some(("_".to_string(), wild.underscore_token.span())),
        _ => None,
    }
}

fn check_match_arm_lexicographically_ordered(arms: &[Arm]) -> Result<(), (String, Span)> {
    if arms.is_empty() {
        return Ok(());
    }

    let Some((mut prev_name, span)) = match_arm_pattern_sort_key(&arms[0].pat) else {
        return Err(("unsupported by #[sorted]".to_string(), arms[0].pat.span()));
    };

    if prev_name.eq("_") && arms.len() != 1 {
        return Err((
            "wildcard cannot be present before other arms".to_string(),
            span,
        ));
    }

    for (ix, arm) in arms.iter().skip(1).enumerate() {
        let Some((current_name, span)) = match_arm_pattern_sort_key(&arm.pat) else {
            return Err((
                "unsupported pattern in #[sorted] match".to_string(),
                arm.pat.span(),
            ));
        };
        if current_name.eq("_") && ix + 1 != arms.len() - 1 {
            return Err((
                format!(
                    "wildcard cannot be present before other arms: ix: {}, arm_len: {}",
                    ix,
                    arms.len()
                ),
                span,
            ));
        }
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
