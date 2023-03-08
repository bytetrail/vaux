use proc_macro::{self, TokenStream};
use quote::quote;
use syn::{parse_macro_input, Data, DataStruct, DeriveInput, Fields, Generics, Ident};

#[proc_macro_derive(PropertyEncode)]
pub fn property_encode(input: TokenStream) -> TokenStream {
    let input: DeriveInput = parse_macro_input!(input);
    derive_property_encoder(&input)
}

#[proc_macro_derive(PropertySize)]
pub fn property_size(input: TokenStream) -> TokenStream {
    let input: DeriveInput = parse_macro_input!(input);
    derive_property_size(&input)
}

fn derive_property_encoder(input: &DeriveInput) -> TokenStream {
    let ident = &input.ident;
    let generics = &input.generics;
    let _pkg_name = std::env::var("CARGO_PKG_NAME").ok().unwrap_or_default();

    match input.data {
        Data::Struct(DataStruct {
            fields: Fields::Named(ref _fields),
            ..
        }) => encoder_impl(ident, generics),
        Data::Struct(DataStruct {
            fields: Fields::Unnamed(_),
            ..
        }) => todo!(),
        Data::Struct(DataStruct {
            fields: Fields::Unit,
            ..
        }) => todo!(),
        Data::Enum(_) => todo!(),
        Data::Union(_) => todo!(),
    }
}

fn derive_property_size(input: &DeriveInput) -> TokenStream {
    let ident = &input.ident;
    let generics = &input.generics;
    let _pkg_name = std::env::var("CARGO_PKG_NAME").ok().unwrap_or_default();

    match input.data {
        Data::Struct(DataStruct {
            fields: Fields::Named(ref _fields),
            ..
        }) => size_impl(ident, generics),
        Data::Struct(DataStruct {
            fields: Fields::Unnamed(_),
            ..
        }) => todo!(),
        Data::Struct(DataStruct {
            fields: Fields::Unit,
            ..
        }) => todo!(),
        Data::Enum(_) => todo!(),
        Data::Union(_) => todo!(),
    }
}

fn encoder_impl(name: &Ident, generics: &Generics) -> TokenStream {
    let (impl_generics, type_generics, where_clause) = generics.split_for_impl();

    let gen = quote! {
        impl #impl_generics PropertyEncode #type_generics for #name #where_clause{
            fn property_encode() -> Result<(), crate::codec::MQTTCodecError>{
                unimplemented!()
            }
        }
    };
    gen.into()
}

fn size_impl(name: &Ident, generics: &Generics) -> TokenStream {
    let (impl_generics, type_generics, where_clause) = generics.split_for_impl();

    let gen = quote! {
        impl #impl_generics PropertySize #type_generics for #name #where_clause {
            fn property_size_internal() -> u32 {
                5
            }
        }
    };
    gen.into()
}
