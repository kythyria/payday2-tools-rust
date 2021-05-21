use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::{Data, DataStruct, DeriveInput, Fields, parse_macro_input};

#[proc_macro_derive(EnumTryFrom)]
pub fn derive_enum_tryfrom(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
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
            _ => return quote_spanned!{ var.ident.span()=> compile_error!("All variants must have an explicit dscriminant") }
        };
        let name = &var.ident;
        quote!{ #discriminant => ::std::result::Result::Ok(Self::#name), }
    }).collect();

    let name = input.ident;

    let s = quote! {
        impl ::std::convert::TryFrom<usize> for #name {
            type Error = parse_helpers::InvalidDiscriminant;
            fn try_from(src: usize) -> ::std::result::Result<Self, Self::Error> {
                match src {
                    #arms
                    e => ::std::result::Result::Err(parse_helpers::InvalidDiscriminant { discriminant: e })
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
        _ => quote_spanned!{ item.ident.span()=> compile_error!("Can only parse structs") },
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