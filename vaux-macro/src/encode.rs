use quote::quote;
use crate::{compile_error2, has_attribute, is_primitive_type};

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
    let is_property = has_attribute(&field.attrs, "property");
    let mut property_type = None;  
    field
        .attrs
        .iter()
        .find(|attr| attr.path().is_ident("property"))
        .and_then(|attr| {                    
            Some(attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("property_type") {
                    let value: syn::LitStr = meta.value()?.parse()?;                            
                    property_type = Some(value.value());
                    Ok(())
                } else {
                    Err(meta.error("Expected property_type argument"))
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
    _type_path: &syn::TypePath,
    is_optional: bool,
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
    if is_optional {
        quote ! {
            if let Some(v) = self.#field_name.as_ref() {
                #prop_ident_encode
                codec::put_bin(v, dest)?;
            }
        }
    } else  {
        quote ! {
            #prop_ident_encode
            codec::put_bin(&self.#field_name, dest)?;
        }        
    } 
}

pub(crate) fn encode_for_option(
    field_name: &Option<syn::Ident>,
    type_path: &syn::TypePath,
    property_type: Option<proc_macro2::TokenStream>,
) -> proc_macro2::TokenStream {
    let segment = &type_path.path.segments.last().unwrap();
    let generic_args = match &segment.arguments {
        syn::PathArguments::AngleBracketed(args) => &args.args,
        _ => {
            return compile_error2("Unsupported Option type for Encode derive");
        }
    };

    let inner_type = match generic_args.first() {
        Some(syn::GenericArgument::Type(syn::Type::Path(type_path))) => {
            let inner_segment = &type_path.path.segments.last().unwrap().ident;
            inner_segment.to_string()
        }
        _ => {
            return compile_error2("Unsupported Option inner type for Encode derive");
        }
    };

    if is_primitive_type(&inner_type) {
        encode_for_primitive(field_name, &inner_type, true, property_type)
    } else {
        match inner_type.as_str() {
            "String" => encode_for_string(field_name, true, property_type),
            "Vec" => encode_for_vec(field_name, &type_path, true, property_type),
            _ => quote! {
                    if let Some(v) = self.#field_name.as_mut() {
                        v.encode(dest)?;    
                    }
            },
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

