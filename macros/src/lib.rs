mod binaryreader;

use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::{Attribute, Data, DeriveInput, Fields, parse_macro_input};
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
            type Error = crate::util::InvalidDiscriminant;
            fn try_from(src: u32) -> ::std::result::Result<Self, Self::Error> {
                match src {
                    #arms
                    e => ::std::result::Result::Err(crate::util::InvalidDiscriminant { discriminant: e })
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

fn get_attribute<'a>(attrs: &'a Vec<Attribute>, name: &str) -> Option<&'a Attribute> {
    attrs.iter().filter(|i| i.path.segments[0].ident == name).next()
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
                        impl #generics From<#from_type> for #enum_type #generics {
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

#[proc_macro_derive(ItemReader, attributes(read_as, skip_before, tag))]
pub fn derive_itemreader(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let item = syn::parse_macro_input!(item as syn::DeriveInput);
    binaryreader::derive_itemreader(item).into()
}

#[proc_macro]
pub fn tuple_itemreaders(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(item as syn::LitInt);

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
            impl<#(#params: ItemReader<Item=#params,Error=ReadError>),*> ItemReader for (#(#params,)*) {
                type Error = ReadError;
                type Item = Self;

                fn read_from_stream<R: ReadExt>(stream: &mut R) -> Result<Self::Item, Self::Error> {
                    Ok((
                        #(#params::read_from_stream(stream)?,)*
                    ))
                }

                fn write_to_stream<W: WriteExt>(stream: &mut W, item: &Self::Item) -> Result<(), Self::Error> {
                    #( stream.write_item_as::<#params>(&item.#literals)?; )*
                    Ok(())
                }
            }
        };
        impls.push(q);
    }

    let i = quote!{ #(#impls)* };
    i.into()
}

#[proc_macro_derive(WrapsPyAny)]
pub fn derive_wraps_pyany(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let item = syn::parse_macro_input!(item as syn::DeriveInput);

    let name = item.ident;
    let generics = item.generics;
    let lt = match generics.lifetimes().next() {
        Some(lt) => &lt.lifetime,
        None => return quote_spanned! {
            generics.span() => compile_error!("Variants must have one item")
        }.into(),
    };
    let vis = item.vis;
    let has_types = generics.type_params().map(|tp| &tp.ident).next().is_some();
    let phantom = if has_types {
        quote! { , PhantomData }
    }
    else {
        quote! { }
    };

    let (impl_generics, ty_generics, where_cl) = generics.split_for_impl();

    quote!{
        impl #impl_generics #name #ty_generics #where_cl {
            #vis fn wrap(ob: & #lt PyAny) -> Self {
                Self(ob #phantom)
            }
        }
        impl #impl_generics WrapsPyAny<#lt> for #name #ty_generics #where_cl {
            fn py(&self) -> Python<#lt> { self.0.py() }
            fn as_ptr(&self) -> *mut pyo3::ffi::PyObject { self.0.as_ptr() }
            fn as_pyany(&self) -> & #lt PyAny { self.0 }
        }
        impl #impl_generics pyo3::conversion::IntoPy<PyObject> for #name #ty_generics #where_cl{
            fn into_py(self, py: Python<'_>) -> PyObject {
                self.0.into_py(py)
            }
        }
        impl #impl_generics pyo3::conversion::ToPyObject for #name #ty_generics #where_cl {
            fn to_object(&self, py: Python<'_>) -> PyObject {
                self.0.into_py(py)
            }
        }
        impl #impl_generics pyo3::conversion::FromPyObject<#lt> for #name #ty_generics #where_cl {
            fn extract(ob: & #lt PyAny) -> PyResult<Self> {
                Ok(Self::wrap(ob))
            }
        }
    }.into()
}