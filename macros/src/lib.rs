use proc_macro2::TokenStream as TS2;
use proc_macro::TokenStream;
use quote::{quote, quote_spanned};
use syn::{parse_macro_input, DeriveInput, Data};

#[proc_macro_derive(EnumTryFrom)]
pub fn derive_enum_tryfrom(item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    let enumeration = match input.data {
        Data::Enum(e) => e,
        _ => {
            return quote_spanned! {
                input.ident.span()=> compile_error!("Expected an enum")
            }.into();
        }
    };

    let arms: TS2= enumeration.variants.iter().map(|var| {
        let discriminant = match &var.discriminant {
            Some((_, disc)) => disc,
            _ => return quote_spanned!{ var.ident.span()=> compile_error!("All variants must have an explicit dscriminant") }
        };
        let name = &var.ident;
        quote!{ #discriminant => ::std::result::Result::Ok(Self::#name), }
    }).collect();

    let name = input.ident;

    let s = quote! {
        impl ::std::convert::TryFrom<usize> for #name {
            type Error = ();
            fn try_from(src: usize) -> ::std::result::Result<Self, Self::Error> {
                match src {
                    #arms
                    _ => ::std::result::Result::Err(())
                }
            }
        }
    };

    s.into()
}