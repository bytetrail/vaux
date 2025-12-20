use quote::quote;
use crate::{compile_error2, has_attribute, is_primitive_type};

/// Generate encode implementation for struct fields. The struct fields are encoded in
/// the order they are defined with the exception of property length which does not
/// typically appear in the struct fields. The property length is encoded prior to the
    /// properties themselves when proiperties are present.
    pub(crate) fn encode_field(field: &syn::Field) -> proc_macro2::TokenStream {
    let field_name = &field.ident;
    let field_type = &field.ty;
    let is_property = has_attribute(&field.attrs, "property");
    // let property_type = if is_property {
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
    } else if is_property && property_type.as_ref().unwrap().starts_with("***") {
        return compile_error2(format!("Invalid property_type argument in property attribute {}", property_type.as_ref().unwrap()).as_str());
    }

    let field_encode = match field_type {
        syn::Type::Path(type_path) => {
            let segment = &type_path.path.segments.last().unwrap().ident;
            match segment.to_string().as_str() {
                "String" => encode_for_string(&field_name, false, is_property, property_type.as_ref().map(|s| s.as_str())),
                "Vec" => encode_for_vec(&field_name, &type_path),
                "Option" => encode_for_option(&field_name, &type_path, true, is_property, property_type.as_ref().map(|s| s.as_str())),
                type_name => {
                    if is_primitive_type(type_name) {
                        encode_for_primitive(&field_name, type_name, false, is_property, property_type.as_ref().map(|s| s.as_str()))
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

pub(crate) fn encode_for_string(field_name: &Option<syn::Ident>, is_optional: bool, is_property: bool, property_type: Option<&str>) -> proc_macro2::TokenStream {


    let field_name = if is_optional {
        quote! { v }
    } else {
        quote! { self.#field_name }
    };

    let prop_ident = if is_property {
        let property_type_ident = syn::Ident::new(&property_type.unwrap(), proc_macro2::Span::call_site());
         quote! {
            // Encoding logic for String property            
            dest.put_u8(#property_type_ident as u8);
        }
    } else {
        quote! {
        }
    };
    if is_optional {
        quote ! {
            if let Some(v) = &self.#field_name {
                #prop_ident
                codec::put_utf8(v, dest)?;
            }
        }
    } else  {
        quote ! {
            #prop_ident
            codec::put_utf8(#field_name, dest)?;
        }        
    }

}

pub(crate) fn encode_for_vec(
    field_name: &Option<syn::Ident>,
    _type_path: &syn::TypePath,
) -> proc_macro2::TokenStream {
    let encode = quote! {
        codec::put_bin(&self.#field_name, dest)?;
    };
    encode
}

pub(crate) fn encode_for_option(
    field_name: &Option<syn::Ident>,
    type_path: &syn::TypePath,
    is_optional: bool,
    is_property: bool,
    property_type: Option<&str>,
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

    let encode = if is_primitive_type(&inner_type) {
        encode_for_primitive(&None, &inner_type, true, is_property, property_type)
    } else {
        match inner_type.as_str() {
            "String" => quote! {
                    // Encoding logic for Option<String> property
                    codec::put_utf8(v, dest)?;
            },
            "Vec" => quote! {
                    // Encoding logic for Option<Vec> property
                    codec::put_bin(v, dest)?;
            },
            _ => quote! {
                    v.encode(dest)?;
            },
        }
    };

    quote ! {
        if let Some(v) = self.#field_name {
            #encode
        }
    }
}

pub(crate) fn encode_for_primitive(
    field_name: &Option<syn::Ident>,
    field_type: &str,
    is_optional: bool,
    is_property: bool,
    property_type: Option<&str>,
) -> proc_macro2::TokenStream {
   
    let field_name = if is_optional {
        quote! { v }
    } else {
        quote! { self.#field_name }
    };

    // convert the property_type str to an enumeration path
    let property_type_ident = if let Some(prop_type) = property_type {
        let segments: Vec<&str> = prop_type.split("::").collect();
        let mut tokens = proc_macro2::TokenStream::new();
        for (i, segment) in segments.iter().enumerate() {
            let ident = syn::Ident::new(segment, proc_macro2::Span::call_site());
            if i > 0 {
                tokens.extend(quote! { :: });
            }
            tokens.extend(quote! { #ident });
        }
        tokens
    } else {
        quote! {}
    };


    let prop_ident = if is_property {
         quote! {
            // Encoding logic for String property            
            dest.put_u8(#property_type_ident as u8);
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
                dest.put_u8(#field_name);
                },
            
            "u16" => quote!{
                dest.put_u16(#field_name);
            },
            "u32" => quote!{
                dest.put_u32(#field_name);
            },
            "i8" => quote!{
                dest.put_i8(#field_name);
            },
            "i16" => quote!{
                dest.put_i16(#field_name);
            },
            "i32" => quote!{
                dest.put_i32(#field_name);
            },
            "bool" => quote!{
                dest.put_u8(if #field_name { 1 } else { 0 });
            },
            _ => {
                compile_error2("Unsupported primitive type for MQTT codec encoding")
            }
        };

    quote ! {
        #prop_ident
        #encode
    }        
}

pub(crate) fn decode_for_primitive_property() -> proc_macro2::TokenStream {
    let decode = quote! {};

    decode
}
