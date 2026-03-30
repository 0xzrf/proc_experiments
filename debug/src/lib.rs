use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Attribute, DeriveInput};

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

    let field_formatting_tokens = struct_data.fields.iter().map(|field| {
        let Some(field_name) = &field.ident else {
            panic!("expected each field to be a named field");
        };

        let field_name_str = field_name.to_string();
        if let Some(attr_val_result) = extract_attribute_value(&field.attrs) {
            let attribute_debug_style = match attr_val_result {
                Ok(attr_val) => attr_val,
                Err(e) => panic!("{e}"),
            };

            todo!()
        } else {
            quote! {
                .field(#field_name_str, &self.#field_name)
            }
        }
    });

    quote! {
        impl std::fmt::Debug for #name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct(#name_str)
                #(#field_formatting_tokens)*
                .finish()
            }
        }
    }
    .into()
}

fn extract_attribute_value(attrs: &[Attribute]) -> Option<Result<String, String>> {
    let attr = attrs.iter().find(|a| a.path().is_ident("debug"))?;
    let meta: syn::MetaNameValue = match attr.parse_args() {
        Ok(m) => m,
        Err(_) => return Some(Err("couldn't parse to name value".to_string())),
    };
    match meta.value {
        syn::Expr::Lit(expr_lit) => match expr_lit.lit {
            syn::Lit::Str(lit_str) => Some(Ok(lit_str.value())),
            _ => Some(Err("Invalid attribute value".to_string())),
        },
        _ => Some(Err("Invalid attribute value".to_string())),
    }
}
