use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::{Attribute, Data, DataEnum, DataStruct, DeriveInput, Fields, Meta, parse_macro_input};

#[proc_macro_derive(EnumTryFrom)]
pub fn derive_enum_tryfrom(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(item as DeriveInput);

    let name = &input.ident;

    let enumeration = match input.data {
        Data::Enum(e) => e,
        _ => {
            return quote_spanned! {
                input.ident.span()=> compile_error!("Expected an enum")
            }.into();
        }
    };

    let arms: TokenStream = enumeration.variants.iter().map(|var| {
        let discriminant = match &var.discriminant {
            Some((_, disc)) => disc,
            _ => return quote_spanned!{ var.ident.span()=> compile_error!("All variants must have an explicit discriminant") }
        };
        let name = &var.ident;
        quote!{ #discriminant => ::std::result::Result::Ok(Self::#name), }
    }).collect();

    let to_disc_arms = enumeration.variants.iter().map(|var| {
        let discriminant = match &var.discriminant {
            Some((_, disc)) => disc,
            _ => return quote_spanned!{ var.ident.span()=> compile_error!("All variants must have an explicit discriminant") }
        };
        let variant_name = &var.ident;
        quote!{ #name::#variant_name => #discriminant }
    });

    let s = quote! {
        impl ::std::convert::TryFrom<u32> for #name {
            type Error = parse_helpers::InvalidDiscriminant;
            fn try_from(src: u32) -> ::std::result::Result<Self, Self::Error> {
                match src {
                    #arms
                    e => ::std::result::Result::Err(parse_helpers::InvalidDiscriminant { discriminant: e })
                }
            }
        }

        impl ::std::convert::From<#name> for u32 {
            fn from(src: #name) -> u32 {
                match src {
                    #(#to_disc_arms),*
                }
            }
        }
    };

    s.into()
}

#[proc_macro_derive(Parse)]
pub fn derive_parse(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let item = parse_macro_input!(item as DeriveInput);

    match item.data {
        Data::Struct(ref s) => struct_parser(&item, &s),
        Data::Enum(ref e) => enum_parser(&item, &e),
        _ => quote_spanned!{ item.ident.span()=> compile_error!("Can only parse structs or enums") },
    }.into()
}

fn struct_parser(item: &DeriveInput, r#struct: &DataStruct) -> TokenStream {
    let mut parse_lines = Vec::<TokenStream>::new();
    let mut parse_construction = Vec::<TokenStream>::new();
    let mut ser_lines = Vec::<TokenStream>::new();

    match r#struct.fields {
        Fields::Named(ref fields) => { 
            for decl in fields.named.iter() {
                let ident = decl.ident.as_ref().unwrap();
                let readvar_name = format!("readvar_{}", ident);
                let readvar = Ident::new(&readvar_name, Span::call_site());
                let field_type = &decl.ty;

                parse_lines.push(quote! {
                    let (input, #readvar) = <#field_type as parse_helpers::Parse>::parse(input)?
                });
                parse_construction.push(quote!{
                    #ident: #readvar
                });
                ser_lines.push(quote! {
                    <#field_type as parse_helpers::Parse>::serialize(&self.#ident, output)?
                });
            }
        },
        _ => return quote_spanned! {
            item.ident.span()=> compile_error!("Only structs with named fields are currently supported")
        }
    }

    let struct_name = &item.ident;

    return quote! {
        impl parse_helpers::Parse for #struct_name {
            fn parse<'a>(input: &'a [u8]) -> IResult<&'a [u8], Self> {
                #(#parse_lines);*;
                Ok((input, #struct_name {
                    #(#parse_construction),*
                }))
            }

            fn serialize<O: std::io::Write>(&self, output: &mut O) -> std::io::Result<()> {
                #(#ser_lines);*;
                Ok(())
            }
        }
    };
}

fn enum_parser(item: &DeriveInput, r#_enum: &DataEnum) -> TokenStream {
    // For now we assume #[repr(u32)] rather than try and parse that.
    // And assume you've used EnumTryFrom up there.

    let item_name = &item.ident;
    let repr_type = quote!{ u32 };

    quote! {
        impl parse_helpers::Parse for #item_name {
            fn parse<'a>(input: &'a [u8]) -> IResult<&'a [u8], Self> {
                ::nom::combinator::map_res(
                    <#repr_type as parse_helpers::Parse>::parse,
                    <Self as ::std::convert::TryFrom<#repr_type>>::try_from
                )(input)
            }

            fn serialize<O: std::io::Write>(&self, output: &mut O) -> std::io::Result<()> {
                let val = (*self) as u32;
                <#repr_type as parse_helpers::Parse>::serialize(&val, output)
            }
        }
    }
}