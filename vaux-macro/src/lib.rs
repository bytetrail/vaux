mod decode;
mod encode;
mod size;
mod util;

use crate::util::{
    compile_error, get_skip_if_path, has_attribute_with_name_value, is_payload_field,
    is_property_field, skip_field, struct_has_attribute_with_name_value,
};
use crate::{decode::decode_field, size::field_size};
use proc_macro::{self, TokenStream};
use quote::quote;
use syn::{parse_macro_input, ItemStruct};

pub(crate) const CODEC_ATTR: &str = "codec";
pub(crate) const CODEC_ATTR_PROPERTY_TYPE_ARG: &str = "property_type";
pub(crate) const CODEC_ATTR_DECODE_WITH_ARG: &str = "decode_with";
pub(crate) const CODEC_ATTR_ENCODE_WITH_ARG: &str = "encode_with";
pub(crate) const CODEC_ATTR_SKIP_ARG: &str = "skip";
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
    let has_payload =
        struct_has_attribute_with_name_value(fields, CODEC_ATTR, CODEC_ATTR_PAYLOAD_ARG);
    // encode the non-property fields first, then the property fields if any
    let mut encoded = fields
        .named
        .iter()
        .filter_map(|f| {
            if !(is_property_field(&f.attrs).unwrap_or(false)
                || is_payload_field(&f.attrs).unwrap_or(false))
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
            if is_property_field(&f.attrs).unwrap_or(false) {
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
    if has_payload {
        fields.named.iter().for_each(|f| {
            if is_payload_field(&f.attrs).unwrap_or(false) {
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

    let property_field_decode = fields
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

    let decode_properties = if has_properties {
        quote! {
            let (property_length, var_bytes_read) = codec::decode_variable_byte_int(src)?;
            bytes_read += var_bytes_read;
            let mut property_bytes_read = 0;
            while property_bytes_read < property_length {
                let property_type = src.get_u8().try_into()?;
                property_bytes_read += 1;
                match property_type {
                    #(#property_field_decode)*
                    _ => {
                        return Err(codec::MqttCodecError::new_with_kind(format!(
                            "MQTT v5 property type {:?} is not supported",
                            property_type
                        ).as_str(), codec::ErrorKind::UnsupportedProperty(property_type as u8)));
                    }
                }
            }
            bytes_read += property_bytes_read;
        }
    } else {
        quote! {}
    };

    let decode_impl = quote! {
        impl Decode for #struct_name {
            fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<u32, MqttCodecError> {
                use bytes::{BufMut, Buf, BytesMut};
                let mut bytes_read = 0;
                #(#header_field_decoded)*
                #decode_properties
                Ok(bytes_read)
            }
        }
    };

    TokenStream::from(decode_impl)
}
