mod decode;
mod encode;
mod size;
mod util;
use crate::decode::decode_internal;
use crate::size::field_size;
use crate::util::{
    compile_error, filter_attributes, get_packet_type, get_skip_if_path,
    has_attribute_with_name_value, is_payload_field, is_property_field, skip_field,
    struct_has_attribute_with_name_value,
};
use proc_macro::{self, TokenStream};
use quote::quote;
use syn::{parse_macro_input, ItemStruct};

pub(crate) const CODEC_ATTR: &str = "codec";
pub(crate) const PACKET_ATTR: &str = "packet";
pub(crate) const PACKET_ATTR_PACKET_TYPE_ARG: &str = "packet_type";
pub(crate) const CODEC_ATTR_PROPERTY_TYPE_ARG: &str = "property_type";
pub(crate) const CODEC_ATTR_DECODE_WITH_ARG: &str = "decode_with";
pub(crate) const CODEC_ATTR_ENCODE_WITH_ARG: &str = "encode_with";
pub(crate) const CODEC_ATTR_SKIP_ARG: &str = "skip";
pub(crate) const CODEC_ATTR_SKIP_IF_ARG: &str = "skip_if";
pub(crate) const CODEC_ATTR_PAYLOAD_ARG: &str = "payload_type";
//pub(crate) const CODEC_ATTR_PAYLOAD_TYPE_REMAINING: &str = "remaining";
//pub(crate) const CODEC_ATTR_PAYLOAD_TYPE_FIELD: &str = "field";

#[proc_macro_attribute]
pub fn packet(args: TokenStream, input: TokenStream) -> TokenStream {
    let codec_size_impl = derive_codec_size(input.clone());
    let codec_property_size_impl = derive_codec_property_size(input.clone());
    let encode_impl = encode_packet(input.clone());
    let decode_impl = derive_decode(input.clone());

    // get the packet type from the control packet attribute
    let input = parse_macro_input!(input as syn::ItemStruct);
    let struct_attrs = filter_attributes(&input.attrs, &[PACKET_ATTR]);
    let struct_name = &input.ident;
    let struct_vis = &input.vis;

    // get the packet type from the packet attr args
    let nv = parse_macro_input!(args as syn::MetaNameValue);
    let packet_type = get_packet_type(&nv);

    // filter out the codec attributes as no longer needed and add the fixed header
    let mut struct_fields = quote! {
        pub fixed_header: codec::FixedHeader,
    };

    match &input.fields {
        syn::Fields::Named(fields_named) => {
            for field in &fields_named.named {
                let field_attrs = filter_attributes(&field.attrs, &[CODEC_ATTR]);
                let field_vis = &field.vis;
                let field_type = &field.ty;
                let field_name = &field.ident;

                struct_fields = quote! {
                    #struct_fields

                    #(#field_attrs)*
                    #field_vis #field_name: #field_type,
                }
            }
        }
        syn::Fields::Unnamed(_) => {
            return compile_error(
                "packet attribute cannot be applied to tuple structs",
            )
        }
        syn::Fields::Unit => {
            struct_fields = quote! {
                #struct_fields
            }
        }
    }

    let output = quote! {
        #(#struct_attrs)*
        #struct_vis struct #struct_name {
            #struct_fields
        }

        impl #struct_name {
            #struct_vis fn new_with_fixed_header(fixed_header: codec::FixedHeader) -> Result<Self, codec::MqttCodecError> {
                if fixed_header.packet_type != #packet_type {
                    return Err( MqttCodecError::new(
                        format!("Unsuppprted PacketType for {}", "#struct_name").as_str()));
                }
                Ok(Self {
                    fixed_header,
                    ..Default::default()
                })
            }
        }
    };

    TokenStream::from(output)
        .into_iter()
        .chain(codec_size_impl)
        .chain(codec_property_size_impl)
        .chain(encode_impl)
        .chain(decode_impl)
        .collect()
}

#[proc_macro_derive(PropertyCodecSize, attributes(codec))]
pub fn derive_codec_property_size(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ItemStruct);
    let name = &input.ident;
    // ensure that the struct does not have tuple fields
    let fields = match &input.fields {
        syn::Fields::Named(fields_named) => fields_named,
        syn::Fields::Unnamed(_) => {
            return compile_error("Size cannot be used for tuple structs");
        }
        syn::Fields::Unit => {
            // return an empty token stream for unit structs
            return TokenStream::new();
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
        impl codec::PropertyCodecSize for #name {
            fn property_size(&self) -> u32 {
                use codec::CodecSize;

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
/// <em>Note:</em> This macro does not support tuple structs and relies on the
/// 'vaux_mqtt::codec' module for size calculation functions.
#[proc_macro_derive(CodecSize, attributes(codec))]
pub fn derive_codec_size(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ItemStruct);
    let name = &input.ident;

    // ensure that the struct does not have tuple fields
    let fields = match &input.fields {
        syn::Fields::Named(fields_named) => Some(fields_named),
        syn::Fields::Unnamed(_) => {
            return compile_error("Size macro does not support tuple structs");
        }
        syn::Fields::Unit => None,
        
    };
    let has_properties = if let Some(fields) = fields {
        struct_has_attribute_with_name_value(fields, CODEC_ATTR, CODEC_ATTR_PROPERTY_TYPE_ARG)
    } else {
        false
    };
    let mut field_sizes = Vec::new();

    if let Some(fields) = fields {
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
    }
    let size_wrapper = if has_properties {
        quote! {
            impl codec::CodecSize for #name {

                fn codec_size(&self) -> u32 {
                    use codec::PropertyCodecSize;
                    let mut total_size = 0;
                    #(#field_sizes)*
                    let property_size = self.property_size();
                    total_size + property_size + codec::variable_byte_int_size(property_size)
                }
            }
        }
    } else {
        quote! {
            impl codec::CodecSize for #name {
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

#[proc_macro_derive(Encode, attributes(codec))]
pub fn derive_encode(input: TokenStream) -> TokenStream {
    encode_internal(input, false)
}

fn encode_packet(input: TokenStream) -> TokenStream {
    encode_internal(input, true)
}

fn encode_internal(input: TokenStream, as_packet: bool) -> TokenStream {
    let input = parse_macro_input!(input as ItemStruct);
    let struct_name = &input.ident;
    // ensure that the struct does not have tuple fields
    let fields = match &input.fields {
        syn::Fields::Named(fields_named) => Some(fields_named),
        syn::Fields::Unnamed(_) => {
            return compile_error("Encode does not support tuple structs");
        }
        syn::Fields::Unit => {
            // return an empty token stream for unit structs
            None
        }
    };
    // check if any field has the property attribute
    let has_properties = if let Some(fields) = fields {
        struct_has_attribute_with_name_value(fields, CODEC_ATTR, CODEC_ATTR_PROPERTY_TYPE_ARG)
    } else {
        false
    };
    let has_payload = if let Some(fields) = fields {
        struct_has_attribute_with_name_value(fields, CODEC_ATTR, CODEC_ATTR_PAYLOAD_ARG)
    } else {
        false
    };
    // encode the non-property fields first, then the property fields if any
    let mut encoded = if let Some(fields) = fields {
        fields
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
        .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    if has_properties {
        // encode the property length
        let property_length_encoding = quote! {
            codec::encode_variable_byte_int(self.property_size(), dest)?;
        };
        encoded.push(property_length_encoding);
        fields.as_ref().unwrap().named.iter().for_each(|f| {
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
        fields.as_ref().unwrap().named.iter().for_each(|f| {
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
        fields.as_ref().unwrap().named.iter().for_each(|f| {
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

    let packet_encode = if as_packet {
        quote! {
            use codec::{CodecSize, PropertyCodecSize};
            self.fixed_header.encode(dest)?;
            codec::encode_variable_byte_int(self.codec_size(), dest)?;
        }
    } else {
        quote! {}
    };

    let encode_impl = quote! {
        impl codec::Encode for #struct_name {
            fn encode(&mut self, dest: &mut bytes::BytesMut) -> Result<(), MqttCodecError> {
                use bytes::{BufMut, BytesMut};
                #packet_encode
                #(#encoded)*
                Ok(())
            }
        }
    };

    TokenStream::from(encode_impl)
}

#[proc_macro_derive(Decode, attributes(codec_as))]
pub fn derive_decode(input: TokenStream) -> TokenStream {
    decode_internal(input)
}
