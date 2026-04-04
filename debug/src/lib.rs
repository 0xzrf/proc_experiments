//! # `CustomDebug` derive macro
//!
//! This procedural macro implements [`std::fmt::Debug`] for structs with **named fields** by
//! generating a `fmt` implementation that prints a stable, readable shape:
//!
//! ```text
//! StructName { field_name: <formatted value>, other: <formatted value> }
//! ```
//!
//! It is intentionally **not** a wrapper around [`std::fmt::DebugStruct`]; it writes the opening
//! brace, field labels, separators, and closing brace with [`write_str`](std::fmt::Write::write_str)
//! and formats each value with either the default `{:?}` (via [`format_args!`]) or a caller-supplied
//! format string on the field.
//!
//! ## Supported input
//!
//! - Only **`struct`** items. Enums and unions are rejected.
//! - Only structs with **`{ ... }` named fields**. Tuple structs and unit structs are rejected.
//!
//! ## Field attributes: `#[debug = "..."]`
//!
//! The derive is declared with [`attributes(debug)`][attr], so the inert attribute `debug` is
//! reserved for this macro. The **only** supported form per field is a **name–value** attribute
//! whose value is a **string literal** (the format string passed to [`format_args!`] together with
//! the field value):
//!
//! ```rust,ignore
//! use debug::CustomDebug;
//!
//! #[derive(CustomDebug)]
//! pub struct Field {
//!     name: &'static str,
//!     #[debug = "0b{:08b}"] // bitmask printed in binary with padding
//!     bitmask: u8,
//! }
//! ```
//!
//! If `#[debug = "..."]` is present, the expansion uses `format_args!(<lit>, self.<field>)`. You are
//! responsible for choosing a format compatible with the field’s type (same as with `format!`).
//!
//! ## Generics and `Debug` bounds (heuristic)
//!
//! Procedural macros run **before** type checking, so they cannot resolve types or traits by name.
//! This crate therefore infers bounds syntactically from field types:
//!
//! - If a **type parameter** `T` appears as a **simple path** `T` in a field type, the macro adds
//!   `T: Debug` to the impl’s generic parameters.
//! - If a path looks like an **associated type** of a parameter (first segment is a type param and
//!   there is more than one segment, e.g. `T::Value`), the macro adds a **where-clause** predicate
//!   `T::Value: Debug` (using the exact path string from the AST).
//! - **`PhantomData<T>`** is a special case: if a field’s type is **exactly** `PhantomData<T>`
//!   (one angle-bracketed type argument, and that argument is the bare type parameter `T`), that
//!   field is **skipped** when collecting usages. Then **`T: Debug` is not required** for that
//!   parameter, because `PhantomData<T>` implements `Debug` even when `T` does not. This matches the
//!   common pattern of carrying `T` only for variance/phantom purposes.
//!
//! Callers can still construct `YourStruct<T>` with a `T` that does not implement `Debug`; such
//! types simply won’t satisfy the generated impl’s bounds and won’t be usable where `Debug` is
//! required—unless the only use of `T` is through `PhantomData<T>` as above.
//!
//! ### Why not `where FieldType: Debug` for every field?
//!
//! Bounding each field type directly (`Option<Box<Other<T>>>: Debug`, etc.) tends to **cycle** on
//! mutually recursive structs and can **expose private types** in public `where` clauses. Bounding
//! type parameters and associated paths (as this macro does) avoids those failure modes in the
//! usual case; see the test suite comments in `06-bound-trouble.rs` for the detailed explanation.
//!
//! ## Compile-time errors emitted by this macro
//!
//! | Message | When |
//! |--------|------|
//! | `CustomDebug can only be implemented on structs` | Input is an enum or union. |
//! | `CustomDebug expects a struct with named fields` | Tuple struct or unit struct. |
//! | `CustomDebug expects named fields` | Defensive: named fields list contained a field without an ident (should not happen after the named-fields check). |
//! | `expected #[debug = "..."]` | `#[debug]` is not name–value, or path is `debug` with wrong meta shape; or the value is not a string literal. |
//! | `duplicate #[debug = "..."] attribute` | More than one `#[debug = ...]` on the same field. |
//!
//! ## Examples (mirror the `tests/` progression)
//!
//! Basic struct: output starts with the struct name and field names (see `02-impl-debug.rs`):
//!
//! ```rust,ignore
//! use debug::CustomDebug;
//!
//! #[derive(CustomDebug)]
//! pub struct Field {
//!     name: &'static str,
//!     bitmask: u8,
//! }
//!
//! // format!("{:?}", f) begins with: Field { name: "F",
//! ```
//!
//! Generic struct: `T` gets a `Debug` bound when used in a field (see `04-type-parameter.rs`):
//!
//! ```rust,ignore
//! #[derive(CustomDebug)]
//! pub struct Field<T> {
//!     value: T,
//!     #[debug = "0b{:08b}"]
//!     bitmask: u8,
//! }
//! ```
//!
//! `PhantomData` only: `T` is not required to implement `Debug` (see `05-phantom-data.rs`):
//!
//! ```rust,ignore
//! use std::marker::PhantomData;
//!
//! #[derive(CustomDebug)]
//! pub struct Field<T> {
//!     marker: PhantomData<T>,
//!     string: String,
//!     #[debug = "0b{:08b}"]
//!     bitmask: u8,
//! }
//! ```
//!
//! Associated type: `T::Value: Debug` in the where clause (see `07-associated-type.rs`):
//!
//! ```rust,ignore
//! pub trait Trait { type Value; }
//!
//! #[derive(CustomDebug)]
//! pub struct Field<T: Trait> {
//!     values: Vec<T::Value>,
//! }
//! ```
//!
//! [attr]: https://doc.rust-lang.org/reference/procedural-macros.html#derive-macros

use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::punctuated::Punctuated;
use syn::token::Comma;
use syn::{
    parse_macro_input, parse_quote, Attribute, DeriveInput, Field, GenericParam, Generics, Ident,
};

/// Derives [`std::fmt::Debug`] for a public or private struct with named fields.
///
/// # Expansion shape
///
/// The generated `fmt` writes the struct name, ` { `, then for each field in declaration order:
/// a comma separator (except before the first field), the field name, `": "`, and either
/// `format_args!(<custom>, self.<field>)` if `#[debug = ...]` is set, or
/// `format_args!("{:?}", &self.<field>)` otherwise. It ends with ` }`.
///
/// # Attributes
///
/// Only the inert field attribute `debug` is understood, and only as `#[debug = "format"]`.
/// See the [crate documentation](crate) for validation rules and error messages.
///
/// # Generics
///
/// The macro clones the input’s [`Generics`], adds `Debug` bounds (see `add_trait_bounds` below),
/// and uses [`Generics::split_for_impl`] so the impl header matches standard derive output.
///
/// # Examples
///
/// Default `Debug` formatting for every field:
///
/// ```rust,ignore
/// use debug::CustomDebug;
///
/// #[derive(CustomDebug)]
/// struct Point { x: i32, y: i32 }
/// ```
///
/// Custom format for one field (binary):
///
/// ```rust,ignore
/// #[derive(CustomDebug)]
/// struct Byte {
///     #[debug = "0b{:08b}"]
///     bitmask: u8,
/// }
/// ```
///
/// # Rejected inputs (compile errors from this macro)
///
/// ```rust,ignore
/// use debug::CustomDebug;
///
/// enum E { V } // CustomDebug can only be implemented on structs
/// // #[derive(CustomDebug)]
/// // impl E {}
///
/// struct Tuple(u8, u8); // CustomDebug expects named fields — use { f0: u8, f1: u8 } style instead
/// // #[derive(CustomDebug)]
///
/// struct Unit; // unit struct — same error
/// ```
///
/// Invalid `debug` attributes on a field:
///
/// ```rust,ignore
/// #[derive(CustomDebug)]
/// struct Bad {
///     // #[debug] // wrong shape → expected #[debug = "..."]
///     // #[debug(foo)] // ditto
///     // #[debug = 1] // not a string literal → expected #[debug = "..."]
///     x: u8,
/// }
/// ```
#[proc_macro_derive(CustomDebug, attributes(debug))]
pub fn derive(input: TokenStream) -> TokenStream {
    // `DeriveInput`: attrs, vis, ident, generics, data (struct/enum/union).
    let input = parse_macro_input!(input as DeriveInput);

    let name = &input.ident;
    let name_str = name.to_string();
    // Enums/unions: see compile error table in crate docs.
    let syn::Data::Struct(struct_data) = &input.data else {
        return syn::Error::new_spanned(name, "CustomDebug can only be implemented on structs")
            .to_compile_error()
            .into();
    };

    // Tuple structs `struct T(u8);` and unit structs `struct T;` are rejected; see tests / crate docs.
    let fields = match &struct_data.fields {
        syn::Fields::Named(fields) => &fields.named,
        syn::Fields::Unnamed(_) | syn::Fields::Unit => {
            return syn::Error::new_spanned(name, "CustomDebug expects a struct with named fields")
                .to_compile_error()
                .into();
        }
    };

    // Clone generics and add `Debug` bounds / where clauses (PhantomData exception, associated types).
    let generics = add_trait_bounds(input.generics, fields);
    let (impl_generic, type_generic, where_clause) = generics.split_for_impl();

    // One `quote!` fragment per field: optional `", "` then label + formatted value.
    let mut field_stmts = Vec::new();
    for (i, field) in fields.iter().enumerate() {
        let Some(field_name) = &field.ident else {
            return syn::Error::new_spanned(name, "CustomDebug expects named fields")
                .to_compile_error()
                .into();
        };

        let field_name_str = field_name.to_string();

        // At most one `#[debug = "..."]`; invalid or duplicate attrs become `compile_error!`.
        let maybe_format = match extract_debug_format(&field.attrs) {
            Ok(v) => v,
            Err(e) => return e.to_compile_error().into(),
        };

        let comma = if i == 0 {
            quote! {}
        } else {
            quote! { f.write_str(", ")?; }
        };

        // Custom: `format_args!(lit, self.field)` — lit must match field type. Default: `{:?}` on `&field`.
        let value_stmt = if let Some(fmt) = maybe_format {
            quote! {
                f.write_str(#field_name_str)?;
                f.write_str(": ")?;
                f.write_fmt(std::format_args!(#fmt, self.#field_name))?;
            }
        } else {
            quote! {
                f.write_str(#field_name_str)?;
                f.write_str(": ")?;
                f.write_fmt(std::format_args!("{:?}", &self.#field_name))?;
            }
        };

        field_stmts.push(quote! {
            #comma
            #value_stmt
        });
    }

    // Final shape: `Name { field: …, … }` — matches expectations in `02-impl-debug` / `03-custom-format`.
    quote! {
        impl #impl_generic std::fmt::Debug for #name #type_generic #where_clause {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(#name_str)?;
                f.write_str(" { ")?;
                #(#field_stmts)*
                f.write_str(" }")
            }
        }
    }
    .into()
}

/// Parses `#[debug = "..."]` from a field’s attribute list.
///
/// # Rules
///
/// - Walks **all** attributes on the field. Non-`debug` attributes are ignored.
/// - Accepts only [`syn::Meta::NameValue`] whose path is `debug` and whose value is
///   [`syn::Expr::Lit`] containing [`syn::Lit::Str`].
/// - If the path is `debug` but the meta is not name–value (e.g. `#[debug]` or `#[debug(...)]`),
///   returns an error: `expected #[debug = "..."]`.
/// - If more than one valid `#[debug = ...]` is found for the same field, returns
///   `duplicate #[debug = "..."] attribute`.
///
/// # Examples
///
/// Accepted:
///
/// ```rust,ignore
/// // #[debug = "0b{:08b}"]
/// ```
///
/// Rejected:
///
/// ```rust,ignore
/// // #[debug]
/// // #[debug(helper)]
/// // #[debug = 42]
/// ```
fn extract_debug_format(attrs: &[Attribute]) -> Result<Option<syn::LitStr>, syn::Error> {
    let mut found: Option<syn::LitStr> = None;

    for attr in attrs {
        let syn::Meta::NameValue(nv) = &attr.meta else {
            if attr.path().is_ident("debug") {
                return Err(syn::Error::new_spanned(attr, "expected #[debug = \"...\"]"));
            }
            continue;
        };

        if !nv.path.is_ident("debug") {
            continue;
        }

        if found.is_some() {
            return Err(syn::Error::new_spanned(
                attr,
                "duplicate #[debug = \"...\"] attribute",
            ));
        }

        let fmt = match &nv.value {
            syn::Expr::Lit(expr_lit) => match &expr_lit.lit {
                syn::Lit::Str(lit_str) => lit_str.clone(),
                other => {
                    return Err(syn::Error::new_spanned(
                        other,
                        "expected #[debug = \"...\"]",
                    ));
                }
            },
            other => {
                return Err(syn::Error::new_spanned(
                    other,
                    "expected #[debug = \"...\"]",
                ));
            }
        };

        found = Some(fmt);
    }

    Ok(found)
}

/// Adds `std::fmt::Debug` bounds to the impl so generated code only types that can be formatted.
///
/// # Algorithm (synactic)
///
/// 1. Collect the set of **type parameter** idents from `generics`.
/// 2. For each field type, unless the field is recognized by [`is_phantomdata_of_param`], walk the type with
///    [`syn::visit::Visit`] and record:
///    - **Direct** use: a [`syn::TypePath`] whose first segment is a type param, with a single
///      segment and no path arguments on that segment (e.g. `T` in `Vec<T>` — the visitor recurses
///      into `Vec` and finds `T`).
///    - **Associated** use: same first segment is a type param, but the path has **more than one**
///      segment (e.g. `T::Value`). The full path is stringified and later reparsed as a `Type` for
///      a where predicate `that_type: Debug`.
/// 3. For each type parameter, if it appears in **direct**, push `std::fmt::Debug` onto that
///    parameter’s bounds.
/// 4. For each **associated** path string, push `path: std::fmt::Debug` into the where clause.
///
/// # PhantomData
///
/// Fields whose type is exactly `PhantomData<P>` for some type parameter `P` (see
/// [`is_phantomdata_of_param`]) are **not** visited, so `P` does not get a `Debug` bound from that
/// field alone.
///
/// # Examples (conceptual)
///
/// - `struct Foo<T> { x: T }` → `T: Debug` on the impl.
/// - `struct Foo<T: Trait> { x: Vec<T::Value> }` → `T::Value: Debug` in `where`.
/// - `struct Foo<T> { m: PhantomData<T>, s: String }` → no `T: Debug` from `PhantomData<T>`; `String`
///   already implements `Debug`.
///
/// # Panics
///
/// If an associated path string cannot be re-parsed as a [`syn::Type`], the macro panics with
/// `"failed to parse type path"`. Under normal syn output this should not happen.
fn add_trait_bounds(mut generics: Generics, fields: &Punctuated<Field, Comma>) -> Generics {
    use std::collections::HashSet;
    use syn::visit::Visit;

    let type_params: HashSet<Ident> = generics.type_params().map(|tp| tp.ident.clone()).collect();

    #[derive(Default)]
    struct Usage {
        direct: HashSet<Ident>,
        associated: HashSet<String>,
    }

    struct Finder<'a> {
        type_params: &'a HashSet<Ident>,
        usage: Usage,
    }

    impl<'a, 'ast> Visit<'ast> for Finder<'a> {
        fn visit_type_path(&mut self, node: &'ast syn::TypePath) {
            if node.qself.is_none() && node.path.leading_colon.is_none() {
                let segs = &node.path.segments;
                if let Some(first) = segs.first() {
                    if self.type_params.contains(&first.ident) {
                        if segs.len() == 1 && matches!(first.arguments, syn::PathArguments::None) {
                            self.usage.direct.insert(first.ident.clone());
                        } else if segs.len() > 1 {
                            self.usage
                                .associated
                                .insert(node.to_token_stream().to_string());
                        }
                    }
                }
            }

            syn::visit::visit_type_path(self, node);
        }
    }

    let mut finder = Finder {
        type_params: &type_params,
        usage: Usage::default(),
    };

    // Treat `PhantomData<T>` as a special case: it never requires `T: Debug`.
    for field in fields {
        let mut is_phantom = false;
        for param in &type_params {
            if is_phantomdata_of_param(&field.ty, param) {
                is_phantom = true;
                break;
            }
        }
        if !is_phantom {
            finder.visit_type(&field.ty);
        }
    }

    for param in &mut generics.params {
        if let GenericParam::Type(ref mut type_param) = *param {
            if finder.usage.direct.contains(&type_param.ident) {
                type_param.bounds.push(parse_quote!(std::fmt::Debug));
            }
        }
    }

    if !finder.usage.associated.is_empty() {
        let where_clause = generics.make_where_clause();
        for assoc in finder.usage.associated {
            let ty: syn::Type = syn::parse_str(&assoc).expect("failed to parse type path");
            where_clause
                .predicates
                .push(parse_quote!(#ty: std::fmt::Debug));
        }
    }

    generics
}

/// Returns `true` if `ty` is exactly `PhantomData<param>` in the restricted sense used by this crate.
///
/// # Matching rules
///
/// - The type must be a [`syn::Type::Path`].
/// - The **last** segment’s ident must be `PhantomData`.
/// - Arguments must be angle-bracketed with **exactly one** generic argument.
/// - That argument must be [`syn::GenericArgument::Type`], and the inner type must be a path with:
///   - no `Self` qualifier ([`syn::TypePath::qself`]),
///   - no leading `::`,
///   - **exactly one** segment,
///   - that segment’s ident equals `param`,
///   - no generic arguments on that segment (`PathArguments::None`).
///
/// Anything else—`PhantomData<&T>`, `PhantomData<(T,)>`, `crate::PhantomData<T>`, or a type
/// parameter spelled with a path—returns `false`, and the macro will treat the field like a normal
/// field for bound inference (so `T` may still get a `Debug` bound if it appears elsewhere).
///
/// # Example
///
/// ```rust,ignore
/// // For type parameter T, only this shape is recognized:
/// // marker: PhantomData<T>
/// ```
fn is_phantomdata_of_param(ty: &syn::Type, param: &syn::Ident) -> bool {
    use syn::{GenericArgument, PathArguments, Type};

    let Type::Path(type_path) = ty else {
        return false;
    };

    let Some(last) = type_path.path.segments.last() else {
        return false;
    };
    if last.ident != "PhantomData" {
        return false;
    }

    let PathArguments::AngleBracketed(ab) = &last.arguments else {
        return false;
    };

    if ab.args.len() != 1 {
        return false;
    }

    match ab.args.first().unwrap() {
        GenericArgument::Type(Type::Path(inner)) => {
            // Require exactly `T` (one segment) not `some::T`
            inner.qself.is_none()
                && inner.path.leading_colon.is_none()
                && inner.path.segments[0].ident == *param
                && matches!(inner.path.segments[0].arguments, PathArguments::None)
        }
        _ => false,
    }
}
