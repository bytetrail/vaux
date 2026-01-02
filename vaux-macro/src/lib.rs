mod encode;
mod header;
mod size;

use crate::size::field_size;
use proc_macro::{self, TokenStream};
use quote::quote;
use syn::{parse_macro_input, punctuated::Punctuated, ItemStruct, Meta, Token};

#[proc_macro_attribute]
pub fn packet_header(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let property_codec_impl = derive_codec_property_size(item.clone());
    let header_codec_impl = derive_codec_size(item.clone());
    let encode_impl = derive_encode(item.clone());
    let decode_impl = derive_decode(item.clone());

    // filter out the property attribute from the struct fields
    let input = parse_macro_input!(item as syn::ItemStruct);
    let struct_attrs = filter_attributes(&input.attrs, &["packet_header"]);
    let struct_name = &input.ident;
    // get the visibility of the struct
    let visibility = &input.vis;

    let mut struct_fields = quote! {};
    // process the fields
    match &input.fields {
        syn::Fields::Named(fields_named) => {
            for field in &fields_named.named {
                // filter out the property attribute
                let field_attrs = filter_attributes(&field.attrs, &["property"]);
                let field_vis = &field.vis;
                let field_name = field.ident.as_ref().expect("expected field name");
                let field_ty = &field.ty;

                struct_fields = quote! {
                    #struct_fields
                    #(#field_attrs)*
                    #field_vis #field_name: #field_ty,
                };
            }
        }
        _ => {
            return compile_error("packet_header can only be applied to structs with named fields");
        }
    };

    let expanded = quote! {
        #(#struct_attrs)*
        #visibility struct #struct_name {
            #struct_fields
        }
    };

    TokenStream::from(expanded)
        .into_iter()
        .chain(property_codec_impl)
        .chain(header_codec_impl)
        .chain(encode_impl)
        .chain(decode_impl)
        .collect()
}

#[proc_macro_derive(PropertyCodecSize, attributes(codec))]
pub fn derive_codec_property_size(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ItemStruct);
    let name = &input.ident;

    // ensure that the struct has named fields
    let fields = match &input.fields {
        syn::Fields::Named(fields_named) => fields_named,
        _ => {
            return compile_error("Size can only be derived for structs with named fields");
        }
    };

    // Generate size calculation for each field
    let mut field_size_calculations = Vec::new();
    for field in &fields.named {
        if let Ok(true) = has_attribute_with_name_value(&field.attrs, "codec", "property_type") {
            let field_size_calc = field_size(field);
            field_size_calculations.push(field_size_calc);
        }
    }

    let size_wrapper = quote! {
        impl crate::PropertyCodecSize for #name {
            fn property_size(&self) -> u32 {
                let mut property_size = 0;

                #(#field_size_calculations)*

                property_size
            }
        }
    };
    TokenStream::from(size_wrapper)
}

#[proc_macro_derive(CodecSize, attributes(codec, property))]
pub fn derive_codec_size(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ItemStruct);
    let name = &input.ident;

    // ensure that the struct has named fields
    let fields = match &input.fields {
        syn::Fields::Named(fields_named) => fields_named,
        _ => {
            return compile_error("Size can only be derived for structs with named fields");
        }
    };

    let has_properties = struct_has_attribute_with_name_value(fields, "codec", "property_type");
    let mut field_sizes = Vec::new();

    for field in &fields.named {
        if !has_attribute_with_name_value(&field.attrs, "codec", "property_type").unwrap_or(false) {
            let field_size_calc = field_size(field);
            field_sizes.push(field_size_calc);
            continue;
        }
    }

    let size_wrapper = if has_properties {
        quote! {

            impl crate::CodecSize for #name {
                fn codec_size(&self) -> u32 {
                    let mut total_size = 0;

                    #(#field_sizes)*

                    let property_size = self.property_size();
                    total_size + property_size + crate::variable_byte_int_size(property_size)
                }
            }
        }
    } else {
        quote! {
            impl crate::CodecSize for #name {
                fn codec_size(&self) -> u32 {
                    let mut total_size = 0;

                    #(#field_sizes)*

                    total_size
                }
            }
        }
    };

    TokenStream::from(size_wrapper)
}

#[proc_macro_derive(Encode, attributes(property, codec))]
pub fn derive_encode(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ItemStruct);
    let struct_name = &input.ident;
    // ensure that the struct has named fields
    let fields = match &input.fields {
        syn::Fields::Named(fields_named) => fields_named,
        _ => {
            return compile_error("Encode can only be derived for structs with named fields");
        }
    };
    // check if any field has the property attribute
    let has_properties = struct_has_attribute(
        match &input.fields {
            syn::Fields::Named(fields_named) => fields_named,
            _ => {
                return compile_error("Encode can only be derived for structs with named fields");
            }
        },
        "property",
    );
    // encode the non-property fields first, then the property fields if any
    let mut encoded = fields
        .named
        .iter()
        .filter_map(|f| {
            if !has_attribute(&f.attrs, "property") {
                Some(encode::encode_field(f))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    if has_properties {
        // encode the property length
        let property_length_encoding = quote! {
            let property_size = self.property_size();
            codec::put_var_u32(property_size, dest);
        };
        encoded.push(property_length_encoding);
        let mut encoded_properties = fields
            .named
            .iter()
            .filter_map(|f| {
                if has_attribute(&f.attrs, "property") {
                    Some(encode::encode_field(f))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        encoded.append(&mut encoded_properties);
    }

    let encode_impl = quote! {
        impl Encode for #struct_name {
            fn encode(&mut self, dest: &mut bytes::BytesMut) -> Result<(), MqttCodecError> {
                use bytes::{BufMut, BytesMut};

                #(#encoded)*

                Ok(())
            }
        }
    };

    TokenStream::from(encode_impl)
}

#[proc_macro_derive(Decode, attributes(codec_as))]
pub fn derive_decode(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ItemStruct);
    let struct_name = &input.ident;
    // ensure that the struct has named fields
    let _fields = match &input.fields {
        syn::Fields::Named(fields_named) => fields_named,
        _ => {
            return compile_error("Decode can only be derived for structs with named fields");
        }
    };

    let decode_impl = quote! {
        impl Decode for #struct_name {

            fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<(), MqttCodecError> {
                use bytes::{BufMut, BytesMut};
                Ok(())
            }
        }
    };

    TokenStream::from(decode_impl)
}

/// Generates a compile-time error with the given message.
pub(crate) fn compile_error(message: &str) -> TokenStream {
    let error_message = format!("Compile-time error in vaux-macro: {}", message);
    TokenStream::from(quote! {
        compile_error!(#error_message);
    })
}

pub(crate) fn compile_error2(message: &str) -> proc_macro2::TokenStream {
    let error_message = format!("Compile-time error in vaux-macro: {}", message);
    quote! {
        compile_error!(#error_message);
    }
}

/// Checks if the given attribute name exists in the list of attributes. This is used to
/// determine if a specific custom attribute is present on a struct or field. In the Size
/// macro, we might use this to check for attributes that specify a specific type for size
///  calculation.
fn has_attribute(attrs: &[syn::Attribute], name: &str) -> bool {
    attrs.iter().any(|attr| attr.path().is_ident(name))
}

/// Checks if the given attribute with a specific name-value pair exists in the list of attributes.
/// This is useful for attributes that have key-value pairs, allowing us to check for specific
/// configurations within an attribute.
pub(crate) fn has_attribute_with_name_value(
    attrs: &[syn::Attribute],
    name: &str,
    key: &str,
) -> Result<bool, syn::Error> {
    let codec_attrs = attrs
        .iter()
        .filter(|attr| attr.path().is_ident(name))
        .collect::<Vec<_>>();
    for attr in &codec_attrs {
        let nested = attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?;
        for meta in nested {
            if let Meta::NameValue(nv_pair) = meta {
                if nv_pair.path.is_ident(key) {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

pub(crate) fn attribute_with_name_value<'a>(
    attrs: &'a [syn::Attribute],
    name: &str,
    key: &str,
) -> Option<&'a syn::Attribute> {
    for attr in attrs {
        if attr.path().is_ident(name) {
            if let syn::Meta::NameValue(nv_pair) = &attr.meta {
                if nv_pair.path.is_ident(key) {
                    return Some(attr);
                }
            }
        }
    }
    None
}

/// Checks if any of the named fields has the specified attribute.
pub(crate) fn struct_has_attribute(fields: &syn::FieldsNamed, attribute_name: &str) -> bool {
    for field in &fields.named {
        for attr in &field.attrs {
            if attr.path().is_ident(attribute_name) {
                return true;
            }
        }
    }
    false
}

pub(crate) fn struct_has_attribute_with_name_value(
    fields: &syn::FieldsNamed,
    attribute_name: &str,
    key: &str,
) -> bool {
    for field in &fields.named {
        for attr in &field.attrs {
            if attr.path().is_ident(attribute_name) {
                if let Ok(true) = has_attribute_with_name_value(&field.attrs, attribute_name, key) {
                    return true;
                }
            }
        }
    }
    false
}

/// Checks if the given type name is a supported primitive type for size calculation.
/// Currently supports u8, i8, bool, u16, i16, u32, i32, and char.
pub(crate) fn is_primitive_type(type_name: &str) -> bool {
    matches!(
        type_name,
        "u8" | "i8" | "bool" | "u16" | "i16" | "u32" | "i32" | "char"
    )
}

pub(crate) fn codec_as(attrs: &[syn::Attribute]) -> Option<String> {
    let mut codec_as_type = None;
    attrs
        .iter()
        .find(|attr| attr.path().is_ident("codec_as"))
        .and_then(|attr| {
            Some(attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("as") {
                    let value: syn::LitStr = meta.value()?.parse()?;
                    codec_as_type = Some(value.value());
                    Ok(())
                } else {
                    Err(meta.error("Expected property_type argument"))
                }
            }))
        });
    codec_as_type
}

/// Filters out attributes whose path matches any in the exclude list. We use this
/// to remove our custom attributes before passing the struct to other macros.
fn filter_attributes(attrs: &Vec<syn::Attribute>, exclude: &[&str]) -> Vec<syn::Attribute> {
    attrs
        .iter()
        .filter(|attr| !exclude.iter().any(|r| attr.path().is_ident(r)))
        .cloned()
        .collect()
}
