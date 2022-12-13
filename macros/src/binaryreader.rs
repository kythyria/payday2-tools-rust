//use proc_macro::{TokenStream};
use proc_macro2::{Span, Ident, TokenStream};
use quote::{quote, quote_spanned};
use syn::{Attribute, LitInt, Expr};

/* 
We need to make several things

struct Foo(A, B)
read() {
    let f_1 = reader.read_as::<A>()?;
    let f_2 = reader.read_as::<B>()?;
    Foo(f_1, f_2)
}
write() {
    writer.write_as::<A>(item.0)?;
    writer.write_as::<B>(item.1)?;
}

struct Foo {
    writer: A,
    item: B
}
read() {
    let f_1 = reader.read_as::<A>()?;
    let f_2 = reader.read_as::<B>()?;
    Foo{ writer: f_1, item: f_2 }
}
write() {
    writer.write_as::<A>(item.writer)?;
    writer.write_as::<B>(item.item)?;
}

enum Owie {
    Foo { writer: A, item: B }
    Bar(A, B)
}
read() {
    match reader.read_as::<u32>()? {
        0 => {
            let f_1 = reader.read_as::<A>()?;
            let f_2 = reader.read_as::<B>()?;
            Owie::Foo{ writer: f_1, item: f_2 }
        },
        1 => {
            let f_1 = reader.read_as::<A>()?;
            let f_2 = reader.read_as::<B>()?;
            Owie::Bar(f_1, f_2)
        }
        other => BadDiscriminant(other)?
    }
}
write() {
    match item {
        Owie::Foo(f_1, f_2) => {
            writer.write_as::<u32>(0);
            writer.write_as::<A>(f_1)?;
            writer.write_as::<B>(f_2)?;
        },
        Owie::Bar{ writer: f_1, item: f_2 } => {
            writer.write_as::<u32>(0);
            writer.write_as::<A>(f_1)?;
            writer.write_as::<B>(f_2)?;
        }
    }
}

All the readers work the same way: create a numbered var per field, then assemble them.
This has the nice property that it's also the match pattern used to disassemble enum variants.
So we just return the three.
*/

pub fn derive_itemreader(item: syn::DeriveInput) -> proc_macro::TokenStream {
    match item.data {
        syn::Data::Struct(ref s) => rw_struct(&item.ident, &s),
        syn::Data::Enum(ref e) => rw_enum(&item, &e),
        _ => quote_spanned!{ item.ident.span()=> compile_error!("Can only parse structs or enums") },
    }.into()
}

fn rw_enum(item: &syn::DeriveInput, enumer: &syn::DataEnum) -> TokenStream {
    let item_name = &item.ident;
    let disc_ty =  match parse_attribute::<Ident>(&item.attrs, "repr") {
        Ok(Some(t)) => quote! { #t },
        Ok(None) => quote! { u32 },
        Err(e) => return e.into_compile_error()
    };


    let mut read_arms = Vec::<TokenStream>::with_capacity(enumer.variants.len());
    let mut write_arms = Vec::<TokenStream>::with_capacity(enumer.variants.len());

    for variant in &enumer.variants {
        let variant_name = &variant.ident;
        let disc_value = match parse_attribute::<Expr>(&variant.attrs, "tag") {
            Ok(Some(expr)) => expr,
            Ok(None) => match &variant.discriminant {
                Some((_, expr)) => expr.clone(),
                None => return quote_spanned!{ variant.ident.span() => compile_error! { "Enums must have discriminants." } }
            },
            Err(e) => return e.into_compile_error()
        };

        let frw = match fields_rw(quote!{ stream }, quote!{item}, &variant.fields) {
            Err(e) => return e,
            Ok(o) => o
        };
    
        let FieldRw { reader_statements, writer_statements, structor_body } = frw;

        read_arms.push(quote!{
            #disc_value => {
                #(#reader_statements);*;
                Ok(#item_name::#variant_name#structor_body)
            }
        });
        write_arms.push(quote!{
            #item_name::#variant_name#structor_body => {
                stream.write_item_as::<#disc_ty>(&#disc_value)?;
                #(#writer_statements);*;
            }
        })
    }
    
    quote! {
        impl binaryreader::ItemReader for #item_name {
            type Error = binaryreader::ReadError;
            type Item = Self;

            fn read_from_stream<R: binaryreader::ReadExt>(stream: &mut R) -> Result<Self::Item, Self::Error> {
                let discriminant = stream.read_item_as::<#disc_ty>()?;
                match discriminant {
                    #(#read_arms),*,
                    o => return Err(ReadError::BadDiscriminant(std::any::type_name::<#item_name>(), o as u128))
                }
            }

            fn write_to_stream<W: binaryreader::WriteExt>(stream: &mut W, item: &Self::Item) -> Result<(), Self::Error> {
                match item {
                    #(#write_arms),*
                }
                Ok(())
            }
        }
    }
}

fn rw_struct(name: &Ident, struc: &syn::DataStruct) -> TokenStream {
    let frw = match fields_rw(quote!{ stream }, quote!{item}, &struc.fields) {
        Err(e) => return e,
        Ok(o) => o
    };

    let FieldRw { reader_statements, writer_statements, structor_body } = frw;

    quote! {
        impl binaryreader::ItemReader for #name {
            type Error = binaryreader::ReadError;
            type Item = Self;

            fn read_from_stream<R: binaryreader::ReadExt>(stream: &mut R) -> Result<Self::Item, Self::Error> {
                #(#reader_statements);*;
                Ok(Self#structor_body)
            }

            fn write_to_stream<W: binaryreader::WriteExt>(stream: &mut W, item: &Self::Item) -> Result<(), Self::Error> {
                let Self#structor_body = item;
                #(#writer_statements);*;
                Ok(())
            }
        }
    }
}

struct FieldRw {
    reader_statements: Vec<TokenStream>,
    writer_statements: Vec<TokenStream>,
    structor_body: TokenStream
}

fn fields_rw(stream: TokenStream, item: TokenStream, fields: &syn::Fields) -> Result<FieldRw, TokenStream> {
    let empty = Default::default();
    let field_list = match fields {
        syn::Fields::Named(na) => &na.named,
        syn::Fields::Unnamed(un) => &un.unnamed,
        syn::Fields::Unit => &empty,
    };

    struct FieldInfo {
        skip_before: Option<LitInt>,
        wire_type: syn::Type,
        name: syn::Member,
        local_name: Ident
    }

    let mut field_infos = Vec::<FieldInfo>::with_capacity(field_list.len());
    for (idx, field) in field_list.iter().enumerate() {
        let wire_type = match parse_attribute(&field.attrs, "read_as") {
            Ok(Some(s)) => s,
            Ok(None) => field.ty.clone(),
            Err(e) => return Err(e.into_compile_error())
        };
        let skip_before = match parse_attribute::<LitInt>(&field.attrs, "skip_before") {
            Ok(s) => s,
            Err(e) => return Err(e.into_compile_error())
        };
        let local_name = Ident::new(&format!("v_{}", idx), Span::call_site());
        let name = match &field.ident {
            Some(n) => syn::Member::Named(n.clone()),
            None => syn::Member::Unnamed(syn::Index{ index:idx as u32, span: Span::call_site() })
        };
        field_infos.push(FieldInfo{
            skip_before, wire_type, local_name, name
        });
    }

    let mut reader_statements = Vec::<TokenStream>::with_capacity(field_list.len());
    let mut writer_statements = Vec::<TokenStream>::with_capacity(field_list.len());
    let mut structor_parts = Vec::<TokenStream>::with_capacity(field_list.len());

    for field in field_infos {
        let FieldInfo { wire_type, local_name, name, skip_before } = field;
        
        if let Some(s) = skip_before {
            reader_statements.push(quote!{ let mut p = [0u8; #s]; #stream.read_exact(&mut p)? });
            writer_statements.push(quote!{ let p = [0u8; #s]; #stream.write_all(&p)? });
        }

        reader_statements.push(quote!{ let #local_name = #stream.read_item_as::<#wire_type>()? });
        writer_statements.push(quote!{ #stream.write_item_as::<#wire_type>(&#local_name)? });

        match fields {
            syn::Fields::Named(_) => structor_parts.push(quote!{ #name: #local_name }),
            syn::Fields::Unnamed(_) => structor_parts.push(quote!{ #local_name }),
            syn::Fields::Unit => (),
        }
    }

    let structor_body = match fields {
        syn::Fields::Named(_) => quote!{{ #(#structor_parts),* }},
        syn::Fields::Unnamed(_) => quote!{( #(#structor_parts),* )},
        syn::Fields::Unit => quote!{},
    };

    Ok(FieldRw { reader_statements, writer_statements, structor_body })
}

fn get_attribute<'a>(attrs: &'a Vec<Attribute>, name: &str) -> Option<&'a Attribute> {
    attrs.iter().filter(|i| i.path.segments[0].ident == name).next()
}

fn parse_attribute<'a, T: syn::parse::Parse>(attrs: &'a Vec<Attribute>, name: &str) -> syn::Result<Option<T>> {
    if let Some(attr) = get_attribute(attrs, name) {
        attr.parse_args::<T>().map(Some)
    }
    else {
        Ok(None)
    }
}