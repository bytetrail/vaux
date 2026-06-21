use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemStruct, Meta, Token, parse_macro_input, punctuated::Punctuated};
use crate::{CODEC_ATTR, CODEC_ATTR_ENCODE_WITH_ARG, CODEC_ATTR_PAYLOAD_ARG, CODEC_ATTR_PROPERTY_TYPE_ARG, get_skip_if_path, is_payload_field, is_property_field, struct_has_attribute_with_name_value, util::{abbreviated_when_expr, attribute_with_name_value, compile_error, compile_error2, has_attribute_with_name_value, is_primitive_type, payload_type, property_type, skip_field} };

pub(crate) fn property_encode_internal(input: TokenStream, _as_packet: bool) -> TokenStream {
    let input = parse_macro_input!(input as ItemStruct);
    let struct_name = &input.ident;    // ensure that the struct does not have tuple fields
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

    let property_fields = if has_properties {
        let mut encoded = Vec::new();
        // encode the property length
        let property_length_encoding = quote! {
            codec::encode_variable_byte_int(self.property_size(), dest)?;
        };
        encoded.push(property_length_encoding);
        fields.as_ref().unwrap().named.iter().for_each(|f| {
            if is_property_field(&f.attrs).unwrap_or(false) {
                let inner_expr = encode_field(f);
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
        encoded
     } else {
        Vec::new()  
    };

    let encode_impl = quote! {
        impl codec::PropertyEncode for #struct_name {
            fn property_encode(&self, dest: &mut bytes::BytesMut) -> Result<(), MqttCodecError> {
                use codec::{Encode, PropertyCodecSize};
                use bytes::{BufMut, BytesMut};                
                #(#property_fields)*                
                Ok(())
            }
        }
    };

    TokenStream::from(encode_impl)
}

pub(crate) fn encode_internal(input: TokenStream, as_packet: bool) -> TokenStream {
    let input = parse_macro_input!(input as ItemStruct);
    let struct_name = &input.ident;
    let as_packet = as_packet || crate::util::is_as_packet(&input.attrs);
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
    let mut header_fields = Vec::new();
    let mut non_abbreviated_header_fields = Vec::new();
    if let Some(fields) = fields {
        for f in &fields.named {
            if is_property_field(&f.attrs).unwrap_or(false)
                || is_payload_field(&f.attrs).unwrap_or(false)
            {
                continue;
            }
            let is_non_abbrev = crate::util::is_non_abbreviated_field(&f.attrs).unwrap_or(false);
            let encoded = if let Some(skip_path) = get_skip_if_path(&f.attrs) {
                let field_name = f.ident.as_ref().unwrap();
                let field_encode = encode_field(f);
                quote! {
                    if !#skip_path(&self.#field_name) {
                        #field_encode
                    }
                }
            } else {
                encode_field(f)
            };
            if is_non_abbrev {
                non_abbreviated_header_fields.push(encoded);
            } else {
                header_fields.push(encoded);
            }
        }
    }



    let payload_fields = if has_payload {
        fields.as_ref().unwrap().named.iter().filter_map(|f| {
            if is_payload_field(&f.attrs).unwrap_or(false) {
                let inner_expr = encode_field(f);
                if let Some(skip_path) = get_skip_if_path(&f.attrs) {
                    let field_name = f.ident.as_ref().unwrap();
                    Some(quote! {
                        if !#skip_path(&self.#field_name) {
                            #inner_expr
                        }
                    })
                } else {
                    Some(inner_expr)
                }
            } else {
                None
            }
        }).collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    let packet_encode = if as_packet {
        quote! {
            use codec::{CodecSize, PropertyCodecSize};
            self.fixed_header.encode(dest)?;
            codec::encode_variable_byte_int(self.codec_size(), dest)?;
        }
    } else {
        quote! {}
    };

    let abbreviated_when = abbreviated_when_expr(&input.attrs);

    let property_encode = if has_properties && as_packet {
        quote! {
            self.property_encode(dest)?;
        }
    } else {
        quote! {}
    };

    let abbreviated_check = if let Some(abbreviated_expr) = abbreviated_when {
        quote! {
            if #abbreviated_expr {
                return Ok(());
            }
        }
    } else {
        quote! {}
    };

    let encode_impl = quote! {
        impl codec::Encode for #struct_name {
            fn encode(&self, dest: &mut bytes::BytesMut) -> Result<(), MqttCodecError> {
                use bytes::{BufMut, BytesMut};
                use codec::PropertyEncode;
                #packet_encode
                #(#header_fields)*
                #abbreviated_check
                #(#non_abbreviated_header_fields)*
                #property_encode
                #(#payload_fields)*
                Ok(())
            }
        }
    };

    TokenStream::from(encode_impl)
}

/// Generate encode implementation for struct fields. The struct fields are encoded in
/// the order they are defined with the exception of property length which does not
/// appear in the struct fields. The property length is encoded prior to the properties 
/// themselves when properties are present.
/// 
/// The payload fields are typically the last field and are encoded last. The ```codec(payload)```
/// attribute may be used to identify payload fields when they are not the last fields.
pub(crate) fn encode_field(field: &syn::Field) -> proc_macro2::TokenStream {
    let field_name = &field.ident;
    let field_type = &field.ty;
    if skip_field(&field.attrs).unwrap() {
        return quote! {};
    }
    let is_property = has_attribute_with_name_value(&field.attrs, CODEC_ATTR, CODEC_ATTR_PROPERTY_TYPE_ARG).unwrap();
    let encode_with = attribute_with_name_value(&field.attrs, CODEC_ATTR, CODEC_ATTR_ENCODE_WITH_ARG).unwrap();
    let property_type: Option<syn::Path> = property_type(&field.attrs);
    if property_type.is_none() && is_property {
        return compile_error2("Property attribute requires a property_type argument");
    } 
    let _payload_type = payload_type(&field.attrs);

    // convert the property_type str to an enumeration path
    let property_type_ident = if is_property && property_type.is_some() { 
        Some(quote! { #property_type })
    } else {
        None
    };
    let field_encode = match field_type {
        syn::Type::Path(type_path) => {
            let segment = &type_path.path.segments.last().unwrap().ident;
            let optional_field = match segment.to_string().as_str() {
                    "Option" => true,
                    _ => false,
                };
            if encode_with.is_some() {
                encode_for_encode_with(&field_name, &encode_with.unwrap(), optional_field, property_type_ident)
            } else {
                match segment.to_string().as_str() {
                    "String" => encode_for_string(&field_name, optional_field, property_type_ident),
                    "Vec" => encode_for_vec(&field_name, &type_path, optional_field, property_type_ident),
                    "Option" => encode_for_option(&field_name, &type_path, property_type_ident),
                    type_name => {
                        if is_primitive_type(type_name) {
                            encode_for_primitive(&field_name, type_name, optional_field,  property_type_ident)
                        } else {
                            quote! {
                                // Encoding logic for complex property
                                self.#field_name.encode(dest)?;
                            }
                        }
                    }
                }
            }
        }
        _ => {
            return compile_error2("Unsupported field type for Encode derive");
        }
    };
    field_encode
}

pub(crate) fn encode_for_string(field_name: &Option<syn::Ident>, 
    is_optional: bool, 
    property_type: Option<proc_macro2::TokenStream>) -> proc_macro2::TokenStream {

    let prop_ident_encode = if let Some(prop_ident) = property_type {
         quote! {
            // Encoding logic for String property            
            dest.put_u8(#prop_ident as u8);
        }
    } else {
        quote! {
        }
    };
    if is_optional {
        quote ! {
            if let Some(v) = self.#field_name.as_ref() {
                #prop_ident_encode
                codec::encode_string(v, dest)?;
            }
        }
    } else  {
        quote ! {
            #prop_ident_encode
            codec::encode_string(&self.#field_name, dest)?;
        }        
    }

}

pub(crate) fn encode_for_vec(
    field_name: &Option<syn::Ident>,
    type_path: &syn::TypePath,
    is_optional: bool,
    property_type: Option<proc_macro2::TokenStream>,
) -> proc_macro2::TokenStream {
    let field = field_name.as_ref().unwrap();
    let is_property = property_type.is_some();
    let prop_ident_encode = if let Some(prop_ident) = property_type {
         quote! {
            // Encoding logic for String property            
            dest.put_u8(#prop_ident as u8);
        }
    } else {
        quote! {
        }
    };
    let encode_stmt = match &type_path.path.segments.last().unwrap().arguments {
        syn::PathArguments::AngleBracketed(args) => match args.args.first().unwrap() {
            syn::GenericArgument::Type(syn::Type::Path(vec_inner_type_path)) => {
                match vec_inner_type_path.path.segments.last().unwrap().ident.to_string().as_str() {
                    "u8" => if is_optional {
                        quote! {
                        #prop_ident_encode
                        codec::encode_array_field(#field, dest)?;
                    }} else {
                        quote! {
                        #prop_ident_encode
                        codec::encode_array_field(&self.#field, dest)?;
                    } } ,
                    
                    "i8" | "u16" | "i16" | "i32" | "u32" | "u64" | "i64" | "f32" | "f64" | "bool" => {
                        return compile_error2("Unsupported Vec inner type for Encode derive");
                    }
                    _field_type=> if is_optional {
                        quote! {
                            for item in #field {
                                #prop_ident_encode
                                item.encode(dest)?;
                            }}
                    } else {
                        if is_property {
                            quote! {
                            // Encoding logic for Vec<ComplexType> property
                            for item in self.#field.iter() {
                                #prop_ident_encode
                                item.encode(dest)?;
                            }}
                        } else {
                            quote! {
                                // Encoding logic for Vec<ComplexType> property
                                for item in self.#field.iter() {
                                    item.encode(dest)?;
                                }
                            }
                        }
                    }  ,
                }
            }
            _ => {
                return compile_error2("Unsupported Vec inner type for Encode derive");
            }
        },  
        _ => {
            return compile_error2("Unsupported Vec type for Encode derive");
        }
    };

    if is_optional {
        quote ! {
            if let Some(#field) = self.#field.as_ref() {
                #encode_stmt
            }
        }
    } else  {
        quote ! {
            #encode_stmt
        }        
    } 
}

pub(crate) fn encode_for_option(
    field_name: &Option<syn::Ident>,
    type_path: &syn::TypePath,
    property_type: Option<proc_macro2::TokenStream>,
) -> proc_macro2::TokenStream {

    let generic_type = match &type_path.path.segments.last().unwrap().arguments {
         syn::PathArguments::AngleBracketed(args) => args.args.first().unwrap(),
        _ => {
            return compile_error2("Unsupported Option type for Size derive");
        }
    };
    match generic_type {
        syn::GenericArgument::Type(syn::Type::Path(type_path)) => {
            let inner_segment = &type_path.path.segments.last().unwrap().ident;
            match inner_segment.to_string().as_str() {
                "String" => return encode_for_string(field_name, true, property_type),
                "Vec" => return encode_for_vec(field_name, type_path, true, property_type),
                type_name => {
                    if is_primitive_type(type_name) {
                        return encode_for_primitive(field_name, type_name, true, property_type);
                    }
                    else {
                        if property_type.is_some() {
                            return quote! {
                                if let Some(#field_name) = self.#field_name.as_ref() {
                                    // Encoding logic for complex property
                                    dest.put_u8(#property_type as u8);
                                    #field_name.encode(dest)?;
                                }
                            };
                        } else {
                            return quote! {
                                if let Some(#field_name) = self.#field_name.as_ref() {
                                    // Encoding logic for complex property
                                    #field_name.encode(dest)?;
                                }
                            };
                        }
                    }
                }
            }
        }
        _ => {
            return compile_error2("Unsupported Option inner type for Encode derive");
        }
    }
}

pub(crate) fn encode_for_primitive(
    field_name: &Option<syn::Ident>,
    field_type: &str,
    is_optional: bool,
    property_type: Option<proc_macro2::TokenStream>,
) -> proc_macro2::TokenStream {
    let some_field_name = if is_optional {
        quote! { v }
    } else {
        quote! { self.#field_name }
    };
    let prop_ident_encode = if let Some(prop_ident) = property_type {
         quote! {
            // Encoding logic for String property            
            dest.put_u8(#prop_ident as u8);
        }
    } else {
        quote! {
        }
    };

    let encode = 
        // Encoding logic for primitive property
        match field_type {
            "u8" => 
                quote! {
                dest.put_u8(#some_field_name);
                },
            
            "u16" => quote!{
                dest.put_u16(#some_field_name);
            },
            "u32" => quote!{
                dest.put_u32(#some_field_name);
            },
            "i8" => quote!{
                dest.put_i8(#some_field_name);
            },
            "i16" => quote!{
                dest.put_i16(#some_field_name);
            },
            "i32" => quote!{
                dest.put_i32(#some_field_name);
            },
            "bool" => quote!{
                dest.put_u8(if #some_field_name { 1 } else { 0 });
            },
            _ => {
                compile_error2("Unsupported primitive type for MQTT codec encoding")
            }
        };

    if is_optional {
        quote! {
            if let Some(v) = self.#field_name {
                #prop_ident_encode
                #encode
            }
        }
    } else {        
        quote ! {
            #prop_ident_encode
            #encode
        }        
    }


}

fn encode_for_encode_with(
    field_name: &Option<syn::Ident>,
    attr: &syn::Attribute,
    optional_field: bool,
    property_type: Option<proc_macro2::TokenStream>,
) -> proc_macro2::TokenStream {
    let prop_ident_encode = if let Some(prop_ident) = property_type {
         quote! {
            // Encoding logic for String property            
            dest.put_u8(#prop_ident as u8);
        }
    } else {
        quote! {
        }
    };
     match attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated) {
        Ok(nested) => {
            let meta = nested.iter().find_map(|m| {
                if let Meta::NameValue(nv_pair) = m {
                    if nv_pair.path.is_ident(CODEC_ATTR_ENCODE_WITH_ARG) {
                        if let syn::Expr::Lit(lit_expr) = &nv_pair.value {
                            if let syn::Lit::Str(lit_str) = &lit_expr.lit {
                                let path: syn::Path = lit_str.parse().unwrap();
                                if optional_field {
                                    return Some(quote! {
                                        if let Some(f) = self.#field_name.as_ref() {
                                            #prop_ident_encode
                                            #path(&self.#field_name, dest)?;
                                        }
                                    });
                                } else {
                                    return Some(quote! {
                                        #prop_ident_encode
                                        #path(&self.#field_name, dest)?;    
                                    });
                                }
                            }
                        }
                    }
                }
                None
            });
            if let Some(tokens) = meta {
                tokens
            } else {
                compile_error2("Unsupported custom encode attribute for Encode derive")
            }
        }
        Err(_) => compile_error2("Unsupported custom encode attribute for Encode derive"),
    }
}

#[cfg(test)]
mod test {
    use super::*;


    #[test]
    fn test_encode_for_primitive() {
        let field_name = Some(syn::Ident::new("test_field", proc_macro2::Span::call_site()));
        let encode_tokens = encode_for_primitive(&field_name, "u16", false, Some(quote! { TestProperty::PropertyOne }));
        let expected = quote! {
            // Encoding logic for String property            
            dest.put_u8(TestProperty::PropertyOne as u8);
            dest.put_u16(self.test_field);      
        };
        assert_eq!(encode_tokens.to_string(), expected.to_string());
    }   

    #[test]
    fn test_encode_for_option_primitive() {
        let field_name = Some(syn::Ident::new("test_field", proc_macro2::Span::call_site()));
        let type_path: syn::TypePath = syn::parse_str("Option<u8>").unwrap();
        let encode_tokens = encode_for_option(&field_name, &type_path, Some(quote! { TestProperty::PropertyTwo }));
        let expected = quote! {
            if let Some(v) = self.test_field {
                // Encoding logic for String property            
                dest.put_u8(TestProperty::PropertyTwo as u8);
                dest.put_u8(v);    
            }
        };
        assert_eq!(encode_tokens.to_string(), expected.to_string());
    }   

    #[test]
    fn test_encode_for_string() {
        let field_name = Some(syn::Ident::new("test_string", proc_macro2::Span::call_site()));
        let encode_tokens = encode_for_string(&field_name, false, Some(quote! { TestProperty::PropertyThree }));
        let expected = quote! {
            // Encoding logic for String property            
            dest.put_u8(TestProperty::PropertyThree as u8);
            codec::encode_string(&self.test_string, dest)?;      
        };
        assert_eq!(encode_tokens.to_string(), expected.to_string());
    }   

    #[test]
    fn test_encode_for_vec() {
        let field_name = Some(syn::Ident::new("data", proc_macro2::Span::call_site()));
        let type_path: syn::TypePath = syn::parse_str("Vec<u8>").unwrap();
        let encode_tokens = encode_for_vec(&field_name, &type_path, false, Some(quote! { TestProperty::PropertyFour }));
        let expected = quote! {
            // Encoding logic for String property            
            dest.put_u8(TestProperty::PropertyFour as u8);
            codec::encode_array_field(&self.data, dest)?;      
        };
        assert_eq!(encode_tokens.to_string(), expected.to_string());
    }

    #[test]
    fn test_encode_simple_struct_vec() {
        let field_name = Some(syn::Ident::new("data", proc_macro2::Span::call_site()));
        let type_path: syn::TypePath = syn::parse_str("Vec<ComplexType>").unwrap();
        let encode_tokens = encode_for_vec(&field_name, &type_path, false, Some(quote! { TestProperty::PropertyFive }));
        let expected = quote! {
            // Encoding logic for Vec<ComplexType> property
            for item in self.data.iter() {
                // Encoding logic for String property            
                dest.put_u8(TestProperty::PropertyFive as u8);
                item.encode(dest)?;
            }      
        };
        assert_eq!(encode_tokens.to_string(), expected.to_string());

        let encode_tokens = encode_for_vec(&field_name, &type_path, false, None);
        let expected = quote! {
            // Encoding logic for Vec<ComplexType> property
            for item in self.data.iter() {
                item.encode(dest)?;
            }
        };
        assert_eq!(encode_tokens.to_string(), expected.to_string());
    }

    #[test]
    fn test_encode_with_attr() {
        let field_name = Some(syn::Ident::new("custom_field", proc_macro2::Span::call_site()));
        let attr: syn::Attribute = syn::parse_quote!(#[codec(encode_with = "custom_encode_function")]);
        let encode_tokens = encode_for_encode_with(&field_name, &attr, false, Some(quote! { TestProperty::PropertySix }));
        let expected = quote! {
            // Encoding logic for String property            
            dest.put_u8(TestProperty::PropertySix as u8);
            custom_encode_function(&self.custom_field, dest)?;    
        };
        assert_eq!(encode_tokens.to_string(), expected.to_string());
    }

}
