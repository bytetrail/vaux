mod decode;
mod encode;
mod size;

use crate::{decode::decode_field, size::field_size};
use proc_macro::{self, TokenStream};
use quote::quote;
use syn::{parse_macro_input, punctuated::Punctuated, ItemStruct, Meta, Token};

pub(crate) const CODEC_ATTR: &str = "codec";
pub(crate) const CODEC_ATTR_PROPERTY_TYPE_ARG: &str = "property_type";
pub(crate) const CODEC_ATTR_DECODE_WITH_ARG: &str = "decode_with";
pub(crate) const CODEC_ATTR_ENCODE_WITH_ARG: &str = "encode_with";
pub(crate) const CODEC_ATTR_SKIP_IF_ARG: &str = "skip_if";
pub(crate) const CODEC_ATTR_PAYLOAD_ARG: &str = "payload_type";
pub(crate) const CODEC_ATTR_PAYLOAD_TYPE_REMAINING: &str = "remaining";
pub(crate) const CODEC_ATTR_PAYLOAD_TYPE_FIELD: &str = "field";

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
        if let Ok(true) =
            has_attribute_with_name_value(&field.attrs, CODEC_ATTR, CODEC_ATTR_PROPERTY_TYPE_ARG)
        {
            if let Some(skip_path) = get_skip_if_path(&field.attrs) {
                let field_name = field.ident.as_ref().unwrap();
                let field_size_calc = field_size(field);
                field_size_calculations.push(quote! {
                    if !#skip_path(&self.#field_name) {
                        #field_size_calc
                    }
                });
                continue;
            } else {
                let field_size_calc = field_size(field);
                field_size_calculations.push(field_size_calc);
                continue;
            }
        }
    }

    let size_wrapper = quote! {
        impl crate::PropertyCodecSize for #name {
            fn property_size(&self) -> u32 {
                use crate::CodecSize;

                let mut property_size = 0;
                #(#field_size_calculations)*
                property_size
            }
        }
    };
    TokenStream::from(size_wrapper)
}

/// Derives the CodecSize trait for a struct, calculating the total size based on its fields.
/// If the struct contains fields marked as properties, it includes the property size and
/// the variable byte integer size for the property length. The generated implementation
/// iterates over each field, calculating its size according to its type and attributes.
///
/// # Arguments
/// * `input` - A TokenStream representing the struct to derive CodecSize for.
///
/// #Attributes
/// * `codec` - Custom attribute used to specify property types for fields.
///
/// # Returns
/// TokenStream representing the generated implementation.
///
/// <em>Note:</em> This macro currently only supports structs with named fields and relies on the
/// 'vaux_mqtt::codec' module for size calculation functions.
#[proc_macro_derive(CodecSize, attributes(codec))]
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

    let has_properties =
        struct_has_attribute_with_name_value(fields, CODEC_ATTR, CODEC_ATTR_PROPERTY_TYPE_ARG);
    let mut field_sizes = Vec::new();

    for field in &fields.named {
        if !has_attribute_with_name_value(&field.attrs, CODEC_ATTR, CODEC_ATTR_PROPERTY_TYPE_ARG)
            .unwrap_or(false)
        {
            let field_size_calc = field_size(field);
            if let Some(skip_path) = get_skip_if_path(&field.attrs) {
                let field_name = field.ident.as_ref().unwrap();
                field_sizes.push(quote! {
                    if !#skip_path(&self.#field_name) {
                        #field_size_calc
                    }
                });
            } else {
                field_sizes.push(field_size_calc);
            }
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
                    total_size + property_size + codec::variable_byte_int_size(property_size)
                }
            }
        }
    } else {
        quote! {
            impl crate::CodecSize for #name {
                fn codec_size(&self) -> u32 {
                    use crate::CodecSize;
                    let mut total_size = 0;

                    #(#field_sizes)*

                    total_size
                }
            }
        }
    };

    TokenStream::from(size_wrapper)
}

#[proc_macro_derive(Encode, attributes(codec))]
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
    let has_properties =
        struct_has_attribute_with_name_value(fields, CODEC_ATTR, CODEC_ATTR_PROPERTY_TYPE_ARG);
    // encode the non-property fields first, then the property fields if any
    let mut encoded = fields
        .named
        .iter()
        .filter_map(|f| {
            if !has_attribute_with_name_value(&f.attrs, CODEC_ATTR, CODEC_ATTR_PROPERTY_TYPE_ARG)
                .unwrap_or(false)
            {
                if let Some(skip_path) = get_skip_if_path(&f.attrs) {
                    let field_name = f.ident.as_ref().unwrap();
                    let field_encode = encode::encode_field(f);
                    Some(quote! {
                        if !#skip_path(&self.#field_name) {
                            #field_encode
                        }
                    })
                } else {
                    Some(encode::encode_field(f))
                }
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    if has_properties {
        // encode the property length
        let property_length_encoding = quote! {
            codec::encode_variable_byte_int(self.property_size(), dest)?;
        };
        encoded.push(property_length_encoding);
        fields.named.iter().for_each(|f| {
            if has_attribute_with_name_value(&f.attrs, CODEC_ATTR, CODEC_ATTR_PROPERTY_TYPE_ARG)
                .unwrap_or(false)
            {
                let inner_expr = encode::encode_field(f);
                if let Some(skip_path) = get_skip_if_path(&f.attrs) {
                    let field_name = f.ident.as_ref().unwrap();
                    encoded.push(quote! {
                        if !#skip_path(&self.#field_name) {
                            #inner_expr
                        }
                    })
                } else {
                    encoded.push(inner_expr);
                }
            }
        });
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
    let fields = match &input.fields {
        syn::Fields::Named(fields_named) => fields_named,
        _ => {
            return compile_error("Decode can only be derived for structs with named fields");
        }
    };
    // check if any field has the property attribute
    let has_properties =
        struct_has_attribute_with_name_value(fields, CODEC_ATTR, CODEC_ATTR_PROPERTY_TYPE_ARG);

    // decode the non-property, non-payload fields first, then the property fields if any
    let header_field_decoded = fields
        .named
        .iter()
        .filter_map(|f| {
            if !(has_attribute_with_name_value(&f.attrs, CODEC_ATTR, CODEC_ATTR_PROPERTY_TYPE_ARG)
                .unwrap_or(false)
                || has_attribute_with_name_value(&f.attrs, CODEC_ATTR, CODEC_ATTR_PAYLOAD_ARG)
                    .unwrap_or(false))
            {
                Some(decode_field(f))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    let property_field_decoded = fields
        .named
        .iter()
        .filter_map(|f| {
            if has_attribute_with_name_value(&f.attrs, CODEC_ATTR, CODEC_ATTR_PROPERTY_TYPE_ARG)
                .unwrap_or(false)
            {
                Some(decode_field(f))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    let decode_prop_len = if has_properties {
        quote! {
            let property_length = codec::decode_variable_byte_int(src)?;
            let mut property_bytes_read = 0;
            while property_bytes_read < property_length {
                let property_type = src.get_u8().try_into()?;
                property_bytes_read += 1;
                match property_type {
                    #(#property_field_decoded)*
                    _ => {
                        return Err(codec::MqttCodecError::new_with_kind(format!(
                            "MQTT v5 property type {:?} is not supported",
                            property_type
                        ).as_str(), codec::ErrorKind::UnsupportedProperty(property_type as u8)));
                    }
                }
            }
        }
    } else {
        quote! {}
    };

    let property_match_arms = fields
        .named
        .iter()
        .filter_map(|f| {
            if has_attribute_with_name_value(&f.attrs, CODEC_ATTR, CODEC_ATTR_PROPERTY_TYPE_ARG)
                .unwrap_or(false)
            {
                Some(decode_field(f))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    let decode_impl = quote! {
        impl Decode for #struct_name {

            fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<(), MqttCodecError> {
                use bytes::{BufMut, Buf, BytesMut};

                #(#header_field_decoded)*
                #decode_prop_len

                // if property_length > 0 {
                //     while property_bytes_read < property_length {
                //         let property_type = codec::decode_u8(src)?;
                //         property_bytes_read += 1;
                //         match property_type {
                //             #(#property_match_arms)*
                //         }
                //     }
                // }
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

/// Returns the skip_if path if there is a ```skip_if``` in the attributes for the field.
/// The skip_if attribute is used to conditionally skip encoding/decoding/size calculation
/// of a field based on a specific condition. This function extracts the condition specified
/// in the skip_if attribute.
///
/// The path must resolve to  function that is called with &self and returns a bool indicating
/// whether to skip the field or not.
///
pub(crate) fn get_skip_if_path(attrs: &[syn::Attribute]) -> Option<syn::Path> {
    let codec_attrs = attrs
        .iter()
        .filter(|attr| attr.path().is_ident(CODEC_ATTR))
        .collect::<Vec<_>>();
    for attr in &codec_attrs {
        let nested = attr
            .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
            .ok()?;
        for meta in nested {
            if let Meta::NameValue(nv_pair) = meta {
                if nv_pair.path.is_ident(CODEC_ATTR_SKIP_IF_ARG) {
                    if let syn::Expr::Lit(lit_expr) = &nv_pair.value {
                        if let syn::Lit::Str(lit_str) = &lit_expr.lit {
                            let path: syn::Path = lit_str.parse().unwrap();
                            return Some(path);
                        }
                    }
                }
            }
        }
    }
    None
}

/// Checks if the given attribute name exists in the list of attributes. This is used to
/// determine if a specific custom attribute is present on a struct or field. In the Size
/// macro, we might use this to check for attributes that specify a specific type for size
///  calculation.
#[allow(dead_code)]
fn has_attribute(attrs: &[syn::Attribute], name: &str) -> bool {
    attrs.iter().any(|attr| attr.path().is_ident(name))
}

/// Checks if any of the named fields has the specified attribute.
#[allow(dead_code)]
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

/// Checks if the given attribute with a specific name-value pair exists in the list of attributes.
/// This is useful for attributes that have key-value pairs, allowing us to check for specific
/// configurations within an attribute. The codec attribute often contains such key-value pairs to
/// customize behavior for encoding, decoding, or size calculation.
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

/// Retrieves the attribute with the specified name-value pair from the list of attributes.
/// This is useful for attributes that have key-value pairs, allowing us to check for specific
/// configurations within an attribute. The codec attribute often contains such key-value pairs to
/// customize behavior for encoding, decoding, or size calculation.
pub(crate) fn attribute_with_name_value<'a>(
    attrs: &'a [syn::Attribute],
    name: &str,
    key: &str,
) -> Result<Option<&'a syn::Attribute>, syn::Error> {
    let codec_attrs = attrs
        .iter()
        .filter(|attr| attr.path().is_ident(name))
        .collect::<Vec<_>>();
    for attr in &codec_attrs {
        let nested = attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?;
        for meta in nested {
            if let Meta::NameValue(nv_pair) = meta {
                if nv_pair.path.is_ident(key) {
                    return Ok(Some(attr));
                }
            }
        }
    }
    Ok(None)
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
        "u8" | "i8"
            | "u16"
            | "i16"
            | "u32"
            | "i32"
            | "bool"
            | "u64"
            | "i64"
            | "f32"
            | "f64"
            | "char"
    )
}

/// Retrieves the property type path from the attributes if it exists.
pub(crate) fn property_type(attrs: &[syn::Attribute]) -> Option<syn::Path> {
    attrs.iter().find_map(|attr| {
        if attr.path().is_ident(CODEC_ATTR) {
            let nested = attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated);
            nested.unwrap().iter().find_map(|meta| {
                if meta.path().is_ident(CODEC_ATTR_PROPERTY_TYPE_ARG) {
                    if let Meta::NameValue(nv_pair) = meta {
                        if let syn::Expr::Lit(lit_str) = &nv_pair.value {
                            if let syn::Lit::Str(lit_str) = &lit_str.lit {
                                Some(lit_str.parse().unwrap())
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
        } else {
            None
        }
    })
}
