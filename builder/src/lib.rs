use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{
    Data, DeriveInput, GenericArgument, Ident, LitStr, PathArguments, Type, parse_macro_input,
};

#[proc_macro_derive(Builder, attributes(builder))]
pub fn derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = &input.ident;

    let Data::Struct(struct_variants) = &input.data else {
        return syn::Error::new_spanned(
            name,
            "Builder Derive macro can only be derived for structs",
        )
        .to_compile_error()
        .into();
    };

    let setter_vars = struct_variants.fields.iter().map(|variant| {
        let variant_name = &variant.clone().ident.unwrap();
        let variant_type = &variant.ty;

        let mut is_val = (false, "".to_string());
        for attr in &variant.attrs {
            if attr.path().is_ident("builder") {
                let _ = attr.parse_nested_meta(|meta| {
                    if !meta.path.is_ident("each") {
                        panic!("expect nested arg to be \"each\"");
                    }
                    let meta_val: LitStr = meta.value().unwrap().parse().unwrap();
                    is_val = (true, meta_val.value());
                    Ok(())
                });
                break;
            }
        }

        if is_val.0 {
            let fn_name = Ident::new(&is_val.1, Span::call_site());

            let Some(elem_type) = vec_element_type(variant_type) else {
                panic!("couldn't extract the Generic from Iterator");
            };

            return quote! {
                pub fn #fn_name(&mut self, new_val: #elem_type) -> &mut Self {
                    self.#variant_name.push(new_val);
                    self
                }
            };
        }

        quote! {
            pub fn #variant_name(&mut self, new_val: #variant_type) -> &mut Self {
                self.#variant_name = new_val;
                self
            }
        }
    });

    quote! {
        impl #name {
            #(#setter_vars)*
        }
    }
    .into()
}

fn vec_element_type(ty: &Type) -> Option<&Type> {
    let Type::Path(type_path) = ty else {
        return None;
    };
    let segment = type_path.path.segments.last()?;
    if !segment.ident.eq("Vec") {
        return None;
    }
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };
    let GenericArgument::Type(elem) = args.args.first()? else {
        return None;
    };
    Some(elem)
}
