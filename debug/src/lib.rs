use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, parse_quote, Attribute, DeriveInput, GenericParam, Generics};

#[proc_macro_derive(CustomDebug, attributes(debug))]
pub fn derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = &input.ident;
    let name_str = name.to_string();
    let syn::Data::Struct(struct_data) = &input.data else {
        return syn::Error::new_spanned(name, "CustomDebug can only be implemented on structs")
            .to_compile_error()
            .into();
    };

    let fields = match &struct_data.fields {
        syn::Fields::Named(fields) => &fields.named,
        syn::Fields::Unnamed(_) | syn::Fields::Unit => {
            return syn::Error::new_spanned(name, "CustomDebug expects a struct with named fields")
                .to_compile_error()
                .into();
        }
    };

    // only allow implementing this macro if the typed parameters implement std::fmt::Debug
    let generics = add_trait_bounds(input.generics);
    let (impl_generic, type_generic, where_clause) = generics.split_for_impl();

    let mut field_stmts = Vec::new();
    for (i, field) in fields.iter().enumerate() {
        let Some(field_name) = &field.ident else {
            return syn::Error::new_spanned(name, "CustomDebug expects named fields")
                .to_compile_error()
                .into();
        };

        let field_name_str = field_name.to_string();

        let maybe_format = match extract_debug_format(&field.attrs) {
            Ok(v) => v,
            Err(e) => return e.to_compile_error().into(),
        };

        let comma = if i == 0 {
            quote! {}
        } else {
            quote! { f.write_str(", ")?; }
        };

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

fn add_trait_bounds(mut generics: Generics) -> Generics {
    for param in &mut generics.params {
        if let GenericParam::Type(ref mut type_param) = *param {
            type_param.bounds.push(parse_quote!(std::fmt::Debug));
        }
    }
    generics
}
