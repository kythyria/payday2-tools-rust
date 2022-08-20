mod binaryreader;

use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::{Attribute, Data, DataEnum, DataStruct, DeriveInput, Fields, LitInt, Type, parse_macro_input};
use syn::spanned::Spanned;

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

/// Generate the "obvious" parser/serialiser by invoking that of each field in turn.
///
/// The `parse_as` attribute takes the identifier of a wrapper type, which can be
/// `Into` the field's type and vice versa, and invokes *that* as the parser instead.
#[proc_macro_derive(Parse, attributes(parse_as, skip_before))]
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
                let parse_as = match get_parseas(&decl.attrs) {
                    Ok(s) => s,
                    Err(_) => return quote_spanned! { decl.span()=> compile_error!("Malformed parse_as attribute") }
                };

                let ident = decl.ident.as_ref().unwrap();
                let readvar_name = format!("readvar_{}", ident);
                let readvar = Ident::new(&readvar_name, Span::call_site());

                let field_type = decl.ty.clone();
                let wire_type = match parse_as {
                    Some(s) => s,
                    None => decl.ty.clone()
                };

                let skip = match get_skipbefore(&decl.attrs) {
                    Ok(s) => s,
                    Err(_) => return quote_spanned! { decl.span()=> compile_error!("Malformed skip_before attribute") }
                };

                if skip.is_some() {
                    parse_lines.push(quote! {
                        let (input, _) = ::nom::bytes::complete::take(#skip as usize)(input)?
                    });

                    ser_lines.push(quote! {
                        output.write_all(&[0; #skip])?
                    });
                }
                
                parse_lines.push(quote_spanned! { field_type.span()=>
                    let (input, #readvar) = <#wire_type as parse_helpers::WireFormat<#field_type>>::parse_into(input)?
                });

                parse_construction.push(quote!{
                    #ident: #readvar
                });
                ser_lines.push(quote! {
                    <#wire_type as parse_helpers::WireFormat<#field_type>>::serialize_from(&self.#ident, output)?
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
            fn parse<'a>(input: &'a [u8]) -> nom::IResult<&'a [u8], Self> {
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
            fn parse<'a>(input: &'a [u8]) -> nom::IResult<&'a [u8], Self> {
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

#[proc_macro]
pub fn gen_tuple_parsers(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(item as syn::LitInt);

    let count = match input.base10_parse::<usize>() {
        Err(_) => return quote_spanned!{ input.span()=> compile_error!("Invalid number of tuples") }.into(),
        Ok(n) => n
    };

    let mut impls = Vec::<TokenStream>::new();

    for i in 1..count+1 {
        let mut literals = Vec::<TokenStream>::new();
        let mut params = Vec::<TokenStream>::new();
        for i in 0..i {
            let id = Ident::new(&format!("T{}", i), Span::call_site());
            let lit = proc_macro2::Literal::usize_unsuffixed(i);
            params.push(quote!{ #id });
            literals.push(quote!{ #lit })
        }
        let q = quote! {
            impl<#(#params: Parse),*> Parse for (#(#params,)*) {
                fn parse<'a>(input: &'a [u8]) -> nom::IResult<&'a [u8], Self> {
                    tuple(( #(<#params as Parse>::parse, )* ))(input)
                }
                fn serialize<O: std::io::Write>(&self, output: &mut O) -> std::io::Result<()> {
                    #( self.#literals.serialize(output)?; )*
                    Ok(())
                }
            }
        };
        impls.push(q);
    }

    let i = quote!{ #(#impls)* };
    i.into()
}

fn get_attribute<'a>(attrs: &'a Vec<Attribute>, name: &str) -> Option<&'a Attribute> {
    attrs.iter().filter(|i| i.path.segments[0].ident == name).next()
}

fn get_parseas<'a>(attrs: &'a Vec<Attribute>) -> syn::Result<Option<Type>> {
    if let Some(attr) = get_attribute(attrs, "parse_as") {
        attr.parse_args::<Type>().map(Some)
    }
    else {
        Ok(None)
    }
}

fn get_skipbefore<'a>(attrs: &'a Vec<Attribute>) -> syn::Result<Option<LitInt>> {
    if let Some(attr) = get_attribute(attrs, "skip_before") {
        attr.parse_args::<LitInt>().map(Some)
    }
    else {
        Ok(None)
    }
}

#[proc_macro_derive(EnumFromData, attributes(no_auto_from))]
pub fn derive_enum_from_data(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(item as syn::ItemEnum);
    
    let trees = input.variants.iter().map(|variant| {
        if let Some(_) = get_attribute(&variant.attrs, "no_auto_from") {
            return quote!{}
        }
        match &variant.fields {
            Fields::Named(_) => quote! {},
            Fields::Unit => quote!{},
            Fields::Unnamed(fields) => {
                if fields.unnamed.len() != 1 {
                    quote_spanned! {
                        fields.span() => compile_error!("Variants must have one item")
                    }
                }
                else {
                    let span = variant.span();
                    let from_type = &variant.fields.iter().next().unwrap();
                    let enum_type = &input.ident;
                    let variant_name = &variant.ident;
                    let generics = &input.generics;
            
                    quote_spanned! { span => 
                        impl#generics From<#from_type> for #enum_type#generics {
                            fn from(src: #from_type) -> Self {
                                #enum_type::#variant_name(src)
                            }
                        }
                    }
                }
            }
        }
    });
    TokenStream::from(quote!{ #(#trees)* }).into()
}

#[proc_macro_derive(ItemReader, attributes(parse_as, skip_before, tag))]
pub fn derive_itemreader(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let item = syn::parse_macro_input!(item as syn::DeriveInput);
    binaryreader::derive_itemreader(item).into()
}