use proc_macro::TokenStream as ProcTs;
use proc_macro2::{Span, TokenStream as Proc2Ts};
use quote::quote;
use syn::Ident;

pub fn handle_bit_struct_definition() -> ProcTs {
    let mut bit_specifier_vec: Vec<Proc2Ts> = Vec::with_capacity(64);

    for i in 1..=64 {
        let i: usize = i;
        let bit_specifier = Ident::new(&format!("B{}", i), Span::call_site());

        let bit_struct_ts = quote! {
            pub struct #bit_specifier;

            impl Specifier for #bit_specifier {
                const BITS: usize = #i;
            }
        };

        bit_specifier_vec.push(bit_struct_ts);
    }

    quote! {
        #(#bit_specifier_vec)*
    }
    .into()
}
