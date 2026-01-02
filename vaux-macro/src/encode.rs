use quote::quote;
use syn::{Meta, Token, punctuated::Punctuated};
use crate::{attribute_with_name_value, compile_error2, has_attribute_with_name_value, is_primitive_type, };

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
    let is_property = has_attribute_with_name_value(&field.attrs, "codec", "property_type").unwrap();
    let encode_with = attribute_with_name_value(&field.attrs, "codec", "encode_with").unwrap();
    let mut property_type = None;  
    field
        .attrs
        .iter()
        .find(|attr| attr.path().is_ident("codec"))
        .and_then(|attr| {                    
            Some(attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("property_type") {
                    let value: syn::LitStr = meta.value()?.parse()?;                            
                    property_type = Some(value.value());
                    Ok(())
                } else {
                    Ok(())
                }
            }))
        });
    if property_type.is_none() && is_property {
        return compile_error2("Property attribute requires a property_type argument");
    } 

    // convert the property_type str to an enumeration path
    let property_type_ident = if is_property && property_type.is_some() { 
        let segments: Vec<&str> = property_type.as_ref().unwrap().split("::").collect();
        let mut tokens = proc_macro2::TokenStream::new();
        for (i, segment) in segments.iter().enumerate() {
            let ident = syn::Ident::new(segment, proc_macro2::Span::call_site());
            if i > 0 {
                tokens.extend(quote! { :: });
            }
            tokens.extend(quote! { #ident });
        }
        Some(tokens)
    } else {
        None
    };

    let field_encode = match field_type {
        syn::Type::Path(type_path) => {
            let segment = &type_path.path.segments.last().unwrap().ident;
            if encode_with.is_some() {
                let optional_field = match segment.to_string().as_str() {
                    "Option" => true,
                    _ => false,
                };
                encode_for_custom(&field_name, &encode_with.unwrap(), optional_field, property_type_ident)
            } else {
                match segment.to_string().as_str() {
                    "String" => encode_for_string(&field_name, false, property_type_ident),
                    "Vec" => encode_for_vec(&field_name, &type_path, false, property_type_ident),
                    "Option" => encode_for_option(&field_name, &type_path, property_type_ident),
                    type_name => {
                        if is_primitive_type(type_name) {
                            encode_for_primitive(&field_name, type_name, false,  property_type_ident)
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
                codec::put_utf8(v, dest)?;
            }
        }
    } else  {
        quote ! {
            #prop_ident_encode
            codec::put_utf8(&self.#field_name, dest)?;
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
                        codec::put_bin(#field, dest)?;
                    }} else {
                        quote! {
                        #prop_ident_encode
                        codec::put_bin(&self.#field, dest)?;
                    } } ,
                    
                    "u16" | "i16" | "i32" | "u32" | "i8" => {
                        return compile_error2("Unsupported Vec inner type for Encode derive");
                    }
                    field_type=> if is_optional {
                        quote! {
                        println!("{}", #field_type);
                        // Encoding logic for Vec<ComplexType> property
                        for item in #field {
                            #prop_ident_encode
                            item.encode(dest)?;
                        }}
                    } else {
                        quote! {
                        // Encoding logic for Vec<ComplexType> property
                        for item in self.#field.iter_mut() {
                            #prop_ident_encode
                            item.encode(dest)?;
                        }
                    } } ,
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
                                if let Some(v) = self.#field_name.as_mut() {
                                    // Encoding logic for complex property
                                    dest.put_u8(#property_type as u8);
                                    v.encode(dest)?;
                                }
                            };
                        } else {
                            return quote! {
                                if let Some(v) = self.#field_name.as_mut() {
                                    // Encoding logic for complex property
                                    v.encode(dest)?;
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

fn encode_for_custom(
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
                    if nv_pair.path.is_ident("encode_with") {
                        if let syn::Expr::Lit(lit_expr) = &nv_pair.value {
                            if let syn::Lit::Str(lit_str) = &lit_expr.lit {
                                let path: syn::Path = lit_str.parse().unwrap();
                                if optional_field {
                                    return Some(quote! {
                                        if let Some(f) = self.#field_name.as_ref() {
                                            #prop_ident_encode
                                            #path(f, dest)?;
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
            codec::put_utf8(&self.test_string, dest)?;      
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
            codec::put_bin(&self.data, dest)?;      
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
            for item in self.data.iter_mut() {
                // Encoding logic for String property            
                dest.put_u8(TestProperty::PropertyFive as u8);
                item.encode(dest)?;
            }      
        };
        assert_eq!(encode_tokens.to_string(), expected.to_string());

        let encode_tokens = encode_for_vec(&field_name, &type_path, false, None);
        let expected = quote! {
            // Encoding logic for Vec<ComplexType> property
            for item in self.data.iter_mut() {
                item.encode(dest)?;
            }
        };
        assert_eq!(encode_tokens.to_string(), expected.to_string());
    }

    #[test]
    fn test_encode_with_attr() {
        let field_name = Some(syn::Ident::new("custom_field", proc_macro2::Span::call_site()));
        let attr: syn::Attribute = syn::parse_quote!(#[codec(encode_with = "custom_encode_function")]);
        let encode_tokens = encode_for_custom(&field_name, &attr, false, Some(quote! { TestProperty::PropertySix }));
        let expected = quote! {
            // Encoding logic for String property            
            dest.put_u8(TestProperty::PropertySix as u8);
            custom_encode_function(&self.custom_field, dest)?;    
        };
        assert_eq!(encode_tokens.to_string(), expected.to_string());
    }

}
