use crate::{
    CODEC_ATTR, CODEC_ATTR_DECODE_WITH_ARG, CODEC_ATTR_PAYLOAD_ARG, CODEC_ATTR_PAYLOAD_TYPE_FIELD, CODEC_ATTR_PAYLOAD_TYPE_REMAINING, CODEC_ATTR_PROPERTY_TYPE_ARG, skip_field, util::{
        abbreviated_when_expr, attribute_with_name_value, compile_error, compile_error2, has_attribute_with_name_value, is_primitive_type, min_decode_size, payload_type, property_type, struct_has_attribute_with_name_value
    }
};
use proc_macro::TokenStream;
use quote::quote;
use syn::{Meta, Token, parse_macro_input, punctuated::Punctuated};

pub(crate) fn decode_internal(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as syn::ItemStruct);
    let struct_name = &input.ident;
    // get the struct attributes to check struct level attributes like min_decode_remaining
    let struct_attrs = &input.attrs;
    let min_decode_size = min_decode_size(struct_attrs);
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

    let has_payload = if let Some(fields) = fields {
        struct_has_attribute_with_name_value(fields, CODEC_ATTR, CODEC_ATTR_PAYLOAD_ARG) }
    else {
        false
    };

    // decode the non-property, non-payload fields first, then the property fields if any
    let mut header_field_decoded = Vec::new();
    let mut non_abbreviated_header_decoded = Vec::new();
    if let Some(fields) = fields {
        for f in &fields.named {
            if has_attribute_with_name_value(&f.attrs, CODEC_ATTR, CODEC_ATTR_PROPERTY_TYPE_ARG)
                .unwrap_or(false)
                || has_attribute_with_name_value(&f.attrs, CODEC_ATTR, CODEC_ATTR_PAYLOAD_ARG)
                    .unwrap_or(false)
            {
                continue;
            }
            let is_non_abbrev = crate::util::is_non_abbreviated_field(&f.attrs).unwrap_or(false);
            if is_non_abbrev {
                non_abbreviated_header_decoded.push(decode_field(f));
            } else {
                header_field_decoded.push(decode_field(f));
            }
        }
    }
    
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
            let property_length = property_length as usize;
            bytes_read += var_bytes_read;
            let mut property_bytes_read = 0_usize;
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

    let decode_field_payload = if has_payload {
        // decode any fields with the payload_type argument  set to "field" after the properties
        if let Some(fields) = fields {
            fields
                .named
                .iter()
                .filter_map(|f| {
                        // get the value of the payload_type argument
                        if let Some(payload_type) = payload_type(&f.attrs) {
                            if payload_type == CODEC_ATTR_PAYLOAD_TYPE_FIELD {
                                Some(decode_field(f))
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    })
                .collect::<Vec<_>>()    
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    let decode_remaining_payload = if has_payload {
        if let Some(fields) = fields {
            fields
                .named
                .iter()
                .filter_map(|f| {
                        // get the value of the payload_type argument
                        if let Some(payload_type) = payload_type(&f.attrs) {
                            if payload_type == CODEC_ATTR_PAYLOAD_TYPE_REMAINING {
                                Some(decode_field(f))
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    })
                .collect::<Vec<_>>()    
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    if decode_remaining_payload.len() > 1 {
        return compile_error("Only one field can be marked with payload_type = \"remaining\"");
    }

    let min_size_check = if let Some(min_size)= min_decode_size {
        quote! {
            let required_remaining = if bytes_read < #min_size {
                #min_size - bytes_read
            } else {
                0
            };
            if src.remaining() < required_remaining {
                return Err(codec::MqttCodecError::new_with_kind(
                    format!("Insufficient data for decoding {}: expected at least {} bytes, got {}", stringify!(#struct_name), #min_size, src.remaining()).as_str(),
                    codec::ErrorKind::InsufficientData(required_remaining , src.remaining() as usize),
                ));
            } else if src.remaining() == 0 && bytes_read == #min_size {
                return Ok(bytes_read);
            }
        }
    } else {
        quote! {}
    };

    let min_decode_len = if let Some(min_size) = min_decode_size {
        quote! { 
            let mut min_decode_len = #min_size;
        }
    } else {
        quote! {  }
    };

    let abbreviated_when = abbreviated_when_expr(&input.attrs);
    let abbreviated_check = if abbreviated_when.is_some() {
        quote! {
            if !src.has_remaining() {
                return Ok(bytes_read);
            }
        }
    } else {
        quote! {}
    };

    let decode_impl = quote! {
        impl codec::Decode for #struct_name {
            fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<usize, MqttCodecError> {
                use bytes::{BufMut, Buf, BytesMut};
                let mut bytes_read = 0_usize;
                #min_decode_len
                #min_size_check
                #(#header_field_decoded)*
                #abbreviated_check
                #(#non_abbreviated_header_decoded)*
                #min_size_check
                #decode_properties
                #(#decode_field_payload)*
                #(#decode_remaining_payload)*
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
                    optional_field,
                    &property_type,
                )
            } else {
                match segment.to_string().as_str() {
                    "String" => decode_for_string(field_name, optional_field, &property_type),
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
    _property_type: &Option<syn::Path>,
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
                                let mut value = Vec::new();
                                let var_bytes_read = value.decode(src)?;
                                bytes_read += var_bytes_read;  
                                self.#field_name = Some(value);
                            }
                        } else {
                            quote! {
                                let mut value = Vec::new();
                                let var_bytes_read = value.decode(src)?;
                                bytes_read += var_bytes_read;
                                self.#field_name = value;
                            }
                        }
                    }
                    "i8" | "u16" | "i16" | "u32" | "i32" | "u64" | "i64" | "f32" | "f64"
                    | "bool" => compile_error2(
                        "Decode derive does not support Vec of primitive types except Vec<u8>",
                    ),
                    "String" => {
                        if optional_field {
                            quote! {
                                let mut value = Vec::new();
                                while src.has_remaining() {
                                    let mut item = String::new();
                                    bytes_read += item.decode(src)?;
                                    value.push(item);
                                }
                                self.#field_name = Some(value);
                            }
                        } else {
                            quote! {
                                let mut value = Vec::new();
                                while src.has_remaining() {
                                    let mut item = String::new();
                                    bytes_read += item.decode(src)?;
                                    value.push(item);
                                }
                                self.#field_name = value;
                            }
                        }
                    }
                    _type_name =>                     
                    quote ! {
                        let mut value = Vec::new();
                        while src.has_remaining() {
                            let mut item = #vec_inner_type_path::default();
                            bytes_read += item.decode(src)?;
                            value.push(item);
                        }
                        self.#field_name = value;
                    }
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
    _optional_field: bool,
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
        quote! {
            let (value, decode_bytes_read) = #decode_with(src)?;
            property_bytes_read += decode_bytes_read;
            self.#field_name = value;
        }
    } else {
        quote! {
            let (value, decode_bytes_read) = #decode_with(src)?;
            bytes_read += decode_bytes_read;
            self.#field_name = value;
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use quote::quote;

    #[test]
    fn test_decode_with_non_optional() {
        let field_name = syn::Ident::new("custom_field", proc_macro2::Span::call_site());
        let attr: syn::Attribute = syn::parse_quote!(#[codec(decode_with = "custom_decode_fn")]);
        let tokens = decode_for_decode_with(&field_name, &attr, false, &None);
        let expected = quote! {
            let (value, decode_bytes_read) = custom_decode_fn(src)?;
            bytes_read += decode_bytes_read;
            self.custom_field = value;
        };
        assert_eq!(tokens.to_string(), expected.to_string());
    }

    #[test]
    fn test_decode_with_optional() {
        let field_name = syn::Ident::new("opt_field", proc_macro2::Span::call_site());
        let attr: syn::Attribute = syn::parse_quote!(#[codec(decode_with = "decode_opt_value")]);
        let tokens = decode_for_decode_with(&field_name, &attr, true, &None);
        let expected = quote! {
            let (value, decode_bytes_read) = decode_opt_value(src)?;
            bytes_read += decode_bytes_read;
            self.opt_field = value;
        };
        assert_eq!(tokens.to_string(), expected.to_string());
    }

    #[test]
    fn test_decode_with_property() {
        let field_name = syn::Ident::new("prop_field", proc_macro2::Span::call_site());
        let attr: syn::Attribute = syn::parse_quote!(#[codec(decode_with = "decode_prop_value")]);
        let prop_type: syn::Path = syn::parse_str("PropertyType::TestProp").unwrap();
        let tokens = decode_for_decode_with(&field_name, &attr, false, &Some(prop_type));
        let expected = quote! {
            let (value, decode_bytes_read) = decode_prop_value(src)?;
            property_bytes_read += decode_bytes_read;
            self.prop_field = value;
        };
        assert_eq!(tokens.to_string(), expected.to_string());
    }

    #[test]
    fn test_decode_for_string_non_optional() {
        let field_name = syn::Ident::new("name", proc_macro2::Span::call_site());
        let tokens = decode_for_string(&field_name, false, &None);
        let expected = quote! {
            bytes_read += self.name.decode(src)?;
        };
        assert_eq!(tokens.to_string(), expected.to_string());
    }

    #[test]
    fn test_decode_for_string_optional() {
        let field_name = syn::Ident::new("name", proc_macro2::Span::call_site());
        let tokens = decode_for_string(&field_name, true, &None);
        let expected = quote! {
            let mut value = String::new();
            bytes_read += value.decode(src)?;
            self.name = Some(value);
        };
        assert_eq!(tokens.to_string(), expected.to_string());
    }

    #[test]
    fn test_decode_for_string_property() {
        let field_name = syn::Ident::new("reason_string", proc_macro2::Span::call_site());
        let prop_type: syn::Path = syn::parse_str("PropertyType::ReasonString").unwrap();
        let tokens = decode_for_string(&field_name, true, &Some(prop_type));
        let expected = quote! {
            let mut value = String::new();
            property_bytes_read += value.decode(src)?;
            self.reason_string = Some(value);
        };
        assert_eq!(tokens.to_string(), expected.to_string());
    }

    #[test]
    fn test_decode_for_primitive_u16() {
        let field_name = syn::Ident::new("packet_id", proc_macro2::Span::call_site());
        let field_type: syn::Type = syn::parse_str("u16").unwrap();
        let tokens = decode_for_primitive(&field_name, &field_type, false, &None);
        let expected = quote! {
            self.packet_id = src.get_u16();
            bytes_read += 2;
        };
        assert_eq!(tokens.to_string(), expected.to_string());
    }

    #[test]
    fn test_decode_for_primitive_optional_u32() {
        let field_name = syn::Ident::new("expiry", proc_macro2::Span::call_site());
        let field_type: syn::Type = syn::parse_str("u32").unwrap();
        let tokens = decode_for_primitive(&field_name, &field_type, true, &None);
        let expected = quote! {
            self.expiry = Some(src.get_u32());
            bytes_read += 4;
        };
        assert_eq!(tokens.to_string(), expected.to_string());
    }

    #[test]
    fn test_decode_for_primitive_property() {
        let field_name = syn::Ident::new("max_qos", proc_macro2::Span::call_site());
        let field_type: syn::Type = syn::parse_str("u8").unwrap();
        let prop_type: syn::Path = syn::parse_str("PropertyType::MaxQoS").unwrap();
        let tokens = decode_for_primitive(&field_name, &field_type, true, &Some(prop_type));
        let expected = quote! {
            self.max_qos = Some(src.get_u8());
            property_bytes_read += 1;
        };
        assert_eq!(tokens.to_string(), expected.to_string());
    }
}
