use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::punctuated::Punctuated;
use syn::token::Comma;
use syn::{
    parse_macro_input, parse_quote, Attribute, DeriveInput, Field, GenericParam, Generics, Ident,
};

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
    let generics = add_trait_bounds(input.generics, fields);
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
