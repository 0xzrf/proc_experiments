use crate::*;
use proc_macro2::Span;
use proc_macro2::TokenStream as TokenStream2;
use syn::Ident;
use syn::{punctuated::Punctuated, token::Comma, Field};

pub fn handle_getter_and_setter(
    fields: Punctuated<Field, Comma>,
    struct_size_in_bytes: usize,
) -> BitFieldResult<TokenStream2> {
    let mut return_tokens: Vec<TokenStream2> = Vec::new();

    let mut bit_offset = 0usize;
    for field in &fields {
        let field_ty = &field.ty;
        let field_name = field.ident.as_ref().unwrap().to_string();
        let field_size_in_bits = bits_from_field_type(field_ty)?;
        let fn_return_type = get_return_type_for_bit_size(field_size_in_bits as u8);
        let getter_name = Ident::new(&format!("get_{}", field_name), Span::call_site());
        let setter_name = Ident::new(&format!("set_{}", field_name), Span::call_site());

        let field_get_set_fn_token = quote::quote! {
            pub fn #getter_name(&self) -> #fn_return_type {
                let raw: u64 = {
                    let mut b = [0u8; 8];
                    b[..#struct_size_in_bytes].copy_from_slice(&self.data);
                    u64::from_le_bytes(b)
                };
                let shift = #bit_offset;
                let mask: u64 = if #field_size_in_bits == 64 {
                    u64::MAX
                } else {
                    (1u64 << #field_size_in_bits) - 1
                };
                ((raw >> shift) & mask) as #fn_return_type
            }

            pub fn #setter_name(&mut self, value: #fn_return_type) {
                let mut raw: u64 = {
                    let mut b = [0u8; 8];
                    b[..#struct_size_in_bytes].copy_from_slice(&self.data);
                    u64::from_le_bytes(b)
                };
                let shift = #bit_offset;
                let mask: u64 = if #field_size_in_bits == 64 {
                    u64::MAX
                } else {
                    (1u64 << #field_size_in_bits) - 1
                };
                raw = (raw & !(mask << shift)) | (((value as u64) & mask) << shift);
                self.data.copy_from_slice(&raw.to_le_bytes()[..#struct_size_in_bytes]);
            }
        };
        return_tokens.push(field_get_set_fn_token);
        bit_offset += field_size_in_bits;
    }

    Ok(quote::quote! { #(#return_tokens)* })
}
