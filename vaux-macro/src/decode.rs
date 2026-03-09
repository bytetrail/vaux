use crate::{
    skip_field,
    util::{
        attribute_with_name_value, compile_error, compile_error2, has_attribute_with_name_value,
        is_primitive_type, property_type, struct_has_attribute_with_name_value,
    },
    CODEC_ATTR, CODEC_ATTR_DECODE_WITH_ARG, CODEC_ATTR_PAYLOAD_ARG, CODEC_ATTR_PROPERTY_TYPE_ARG,
};
use proc_macro::TokenStream;
use quote::quote;
use syn::{FieldsNamed, Meta, Token, parse_macro_input, punctuated::Punctuated};

pub(crate) fn decode_internal(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as syn::ItemStruct);
    let struct_name = &input.ident;
    // ensure that the struct does not have tuple fields
    let fields = match &input.fields {
        syn::Fields::Named(fields_named) => Some(fields_named),
        syn::Fields::Unnamed(_) => {
            return compile_error("Decode does not support tuple structs");
        }
        syn::Fields::Unit => {
            // return an empty token stream for unit structs
            None
        }
    };
    // check if any field has the property attribute
    let has_properties = if let Some(fields) = fields {
        struct_has_attribute_with_name_value(fields, CODEC_ATTR, CODEC_ATTR_PROPERTY_TYPE_ARG) }
    else {
        false
    };

    // decode the non-property, non-payload fields first, then the property fields if any
    let header_field_decoded = if let Some(fields) = fields {
        fields.named
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
        .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    
    let property_field_decode = if let Some(fields) = fields {
        fields
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
        .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

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
        impl codec::Decode for #struct_name {
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

pub(crate) fn decode_field(field: &syn::Field) -> proc_macro2::TokenStream {
    let field_name = field.ident.as_ref().unwrap();
    let field_type = field.ty.clone();
    if skip_field(&field.attrs).unwrap_or(false) {
        return quote! {};
    }
    let is_property =
        has_attribute_with_name_value(&field.attrs, CODEC_ATTR, CODEC_ATTR_PROPERTY_TYPE_ARG)
            .unwrap();
    let decode_with =
        attribute_with_name_value(&field.attrs, CODEC_ATTR, CODEC_ATTR_DECODE_WITH_ARG).unwrap();
    let payload_type_attr =
        attribute_with_name_value(&field.attrs, CODEC_ATTR, CODEC_ATTR_PAYLOAD_ARG).unwrap();
    let _payload_type = if let Some(payload_attr) = payload_type_attr {
        match payload_attr.meta {
            syn::Meta::NameValue(ref nv) => match nv.value {
                syn::Expr::Lit(ref expr_lit) => match expr_lit.lit {
                    syn::Lit::Str(ref lit_str) => Some(&lit_str.value().clone()),
                    _ => {
                        return compile_error2(
                            "payload_type attribute value must be a string literal",
                        );
                    }
                },
                _ => {
                    return compile_error2("payload_type attribute value must be a string literal");
                }
            },
            _ => {
                return compile_error2("payload_type attribute must be a name-value pair");
            }
        }
    } else {
        None
    };
    let property_type: Option<syn::Path> = property_type(&field.attrs);
    if property_type.is_none() && is_property {
        return compile_error2("Property attribute requires a property_type argument");
    }
    // Generate the match arm prefix if this is a property
    let field_decode = match &field_type {
        syn::Type::Path(type_path) => {
            //let skip_if = get_skip_if_path(&field.attrs);
            let segment = &type_path.path.segments.last().unwrap().ident;
            let optional_field;
            let mut inner_field_type = field_type.clone();
            match segment.to_string().as_str() {
                "Option" => {
                    match &type_path.path.segments.last().unwrap().arguments {
                        syn::PathArguments::AngleBracketed(args) => {
                            match args.args.first().unwrap() {
                                syn::GenericArgument::Type(syn::Type::Path(
                                    option_inner_type_path,
                                )) => {
                                    inner_field_type =
                                        syn::Type::Path(option_inner_type_path.clone());
                                }
                                _ => {
                                    return compile_error2(
                                        "Unsupported Option inner type for Decode derive",
                                    );
                                }
                            }
                        }
                        _ => {
                            return compile_error2(
                                "Unsupported Option inner type for Decode derive",
                            );
                        }
                    };
                    optional_field = true;
                }
                _ => optional_field = false,
            };
            if decode_with.is_some() {
                decode_for_decode_with(
                    &field_name,
                    &decode_with.unwrap(),
                    
                    &property_type,
                )
            } else {
                match segment.to_string().as_str() {
                    "String" => decode_for_string(field_name, optional_field, &property_type),
                    // "Option" => decode_for_option(&field_name, &type_path, &property_type),
                    "Vec" => {
                        decode_for_vec(&field_name, &type_path, optional_field, &property_type)
                    }
                    type_name => {
                        if is_primitive_type(type_name) {
                            decode_for_primitive(
                                field_name,
                                &inner_field_type,
                                optional_field,
                                &property_type,
                            )
                        } else {
                            decode_for_type(
                                field_name,
                                &inner_field_type,
                                optional_field,
                                &property_type,
                            )
                        }
                    }
                }
            }
        }
        _ => {
            return compile_error2("Unsupported field type for Decode derive");
        }
    };

    let match_arm_wrapper = if is_property {
        quote! {
            #property_type => { #field_decode }
        }
    } else {
        quote! { #field_decode}
    };

    match_arm_wrapper
}

/// Decode logic for String fields. The logic differs based on whether the field is optional
/// and whether it is a property.
fn decode_for_string(
    field_name: &syn::Ident,
    optional_field: bool,
    property_type: &Option<syn::Path>,
) -> proc_macro2::TokenStream {
    let decode_stmt = if property_type.is_some() {
        if optional_field {
            quote! { property_bytes_read += value.decode(src)?; }
        } else {
            quote! { property_bytes_read += self.#field_name.decode(src)?; }
        }
    } else {
        if optional_field {
            quote! { bytes_read += value.decode(src)?; }
        } else {
            quote! { bytes_read += self.#field_name.decode(src)?; }
        }
    };
    if optional_field {
        quote! {
            let mut value = String::new();
            #decode_stmt
            self.#field_name = Some(value);
        }
    } else {
        quote! {
            #decode_stmt
        }
    }
}

fn decode_for_primitive(
    field_name: &syn::Ident,
    field_type: &syn::Type,
    optional_field: bool,
    property_type: &Option<syn::Path>,
) -> proc_macro2::TokenStream {
    let read = if property_type.is_some() {
        quote! { property_bytes_read }
    } else {
        quote! { bytes_read }
    };

    let (reader, bytes_read) = match field_type {
        syn::Type::Path(type_path) => {
            let segment = &type_path.path.segments.last().unwrap().ident;
            match segment.to_string().as_str() {
                "u8" => (quote! {src.get_u8()}, quote! {1}),
                "i8" => (quote! {src.get_i8()}, quote! {1}),
                "u16" => (quote! {src.get_u16()}, quote! {2}),
                "i16" => (quote! {src.get_i16()}, quote! {2}),
                "u32" => (quote! {src.get_u32()}, quote! {4}),
                "i32" => (quote! {src.get_i32()}, quote! {4}),
                "bool" => (quote! {src.get_u8() != 0}, quote! {1}),
                _ => return compile_error2("Unsupported primitive type for Decode derive"),
            }
        }
        _ => return compile_error2("Unsupported primitive type for Decode derive"),
    };

    let assignment = if optional_field {
        quote! {
            self.#field_name = Some( #reader );
            #read += #bytes_read;
        }
    } else {
        quote! {
            self.#field_name = #reader;
            #read += #bytes_read;
        }
    };

    assignment
}

fn decode_for_type(
    field_name: &syn::Ident,
    field_type: &syn::Type,
    optional_field: bool,
    property_type: &Option<syn::Path>,
) -> proc_macro2::TokenStream {
    let decode_stmt = if property_type.is_some() {
        if optional_field {
            quote! { property_bytes_read += value.decode(src)?; }
        } else {
            quote! { property_bytes_read += self.#field_name.decode(src)?; }
        }
    } else {
        if optional_field {
            quote! { bytes_read += value.decode(src)?; }
        } else {
            quote! { bytes_read += self.#field_name.decode(src)?; }
        }
    };

    if optional_field {
        quote! {
            let mut value = #field_type::default();
            #decode_stmt
            self.#field_name = Some(value);
        }
    } else {
        quote! {
            #decode_stmt
        }
    }
}

fn decode_for_vec(
    field_name: &syn::Ident,
    type_path: &syn::TypePath,
    optional_field: bool,
    property_type: &Option<syn::Path>,
) -> proc_macro2::TokenStream {
    let decode_stmt = match &type_path.path.segments.last().unwrap().arguments {
        syn::PathArguments::AngleBracketed(args) => match args.args.first().unwrap() {
            syn::GenericArgument::Type(syn::Type::Path(vec_inner_type_path)) => {
                match vec_inner_type_path
                    .path
                    .segments
                    .last()
                    .unwrap()
                    .ident
                    .to_string()
                    .as_str()
                {
                    "u8" => {
                        if optional_field {
                            quote! {
                                let (value, var_bytes_read) = codec::decode_array_field(src)?;
                                bytes_read += var_bytes_read;
                                self.#field_name = Some(value);
                            }
                        } else {
                            quote! {
                                let (value, var_bytes_read) = codec::decode_array_field(src)?;
                                bytes_read += var_bytes_read;
                                self.#field_name = value;
                            }
                        }
                    }
                    "i8" | "u16" | "i16" | "u32" | "i32" | "u64" | "i64" | "f32" | "f64"
                    | "bool" => compile_error2(
                        "Decode derive does not support Vec of primitive types except Vec<u8>",
                    ),
                    _type_name => decode_for_type(
                        field_name,
                        &syn::Type::Path(vec_inner_type_path.clone()),
                        optional_field,
                        property_type,
                    ),
                }
            }
            _ => {
                return compile_error2("Unsupported Vec inner type for Decode derive");
            }
        },
        _ => {
            return compile_error2("Unsupported Vec inner type for Decode derive");
        }
    };

    quote! {
        #decode_stmt
    }
}

fn decode_for_decode_with(
    field_name: &syn::Ident,
    attr: &syn::Attribute,
    property_type: &Option<syn::Path>,
) -> proc_macro2::TokenStream {
    let decode_path = match attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated) {
        Ok(nested) => nested.iter().find_map(|m| {
            if let Meta::NameValue(nv) = m {
                if nv.path.is_ident(CODEC_ATTR_DECODE_WITH_ARG) {
                    match &nv.value {
                        syn::Expr::Lit(expr_lit) => match &expr_lit.lit {
                            syn::Lit::Str(lit_str) => {
                                let path: syn::Path = lit_str.parse().unwrap();
                                Some(path)
                            }
                            _ => None,
                        },
                        _ => None,
                    }
                } else {
                    None
                }
            } else {
                None
            }
        }),
        _ => {
            return compile_error2("decode_with attribute requires a function path");
        }
    };
    let decode_with = decode_path.unwrap();

    if property_type.is_some() {
        //      if optional_field {
        // quote! {
        //         let( value, decode_bytes_read) = #decode_with(src)?;
        //         property_bytes_read += decode_bytes_read;
        //         self.#field_name = Some(value);
        //     }
        // } else {
        quote! {
            let  (value, decode_bytes_read) = #decode_with(src)?;
            property_bytes_read += decode_bytes_read;
            self.#field_name = value;
        }
        // }
    } else {
        // if optional_field {
        //     quote! {
        //         let (value, decode_bytes_read) = #decode_with(src)?;
        //         bytes_read += decode_bytes_read;
        //         self.#field_name = Some(value);
        //     }
        // } else {
        quote! {
            let (value, decode_bytes_read) = #decode_with(src)?;
            bytes_read += decode_bytes_read;
            self.#field_name = value;
        }
        //        }
    }
}
