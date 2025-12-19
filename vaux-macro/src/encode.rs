use std::any::Any;

use quote::quote;

use crate::{compile_error2, is_primitive_type};

/// Generate encode implementation for struct fields. The struct fields are encoded in
/// the order they are defined with the exception of property length which does not
/// typically appear in the struct fields. The property length is encoded prior to the
/// properties themselves when proiperties are present.
pub(crate) fn encode_field(field: &syn::Field) -> proc_macro2::TokenStream {
    let field_name = &field.ident;
    let field_type = &field.ty;
    let field_type_str = field_type.type_id();
    let attrs = &field.attrs;
    let is_property: bool = crate::has_attribute(attrs, "property");
    let field_encode = match field_type {
        syn::Type::Path(type_path) => {
            let segment = &type_path.path.segments.last().unwrap().ident;
            match segment.to_string().as_str() {
                "String" => encode_for_string(&field_name),
                "Vec" => encode_for_vec(&field_name, &type_path),
                "Option" => encode_for_option(&field_name, &type_path),
                type_name => {
                    if is_primitive_type(type_name) {
                        encode_for_primitive(&field_name, type_name)
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

pub(crate) fn encode_for_string(field_name: &Option<syn::Ident>) -> proc_macro2::TokenStream {
    let encode = quote! {
        // Encoding logic for String property
        codec::put_utf8(&self.#field_name, dest)?;
    };
    encode
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
                // Encoding logic for Option<Primitive> property
                match inner_type.as_str() {
                            "u8" => 
                                quote! {
                                    dest.put_u8(*v);
                                },
                            
                            "u16" => quote! {
                                    dest.put_u16(*v);
                                },
                            "u32" => 
                                quote! {
                                    dest.put_u32(*v);
                                },
                            
                            "i8" => quote!{
                                dest.put_i8(*v);
                            },
                            "i16" => quote!{
                                dest.put_i16(*v);
                            },
                            "i32" => quote!{
                                dest.put_i32(*v);
                            },
                            "bool" => quote!{
                                dest.put_u8(if *v { 1 } else { 0 });
                            },
                            _ => 
                                compile_error2(&format!(
                                    "Unsupported primitive type '{}' for Option encoding",
                                    inner_type
                                ))                            
                        }
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
        if let Some(v) = &self.#field_name {
            #encode
        }
    }
}

pub(crate) fn encode_for_primitive(
    field_name: &Option<syn::Ident>,
    field_type: &str,
) -> proc_macro2::TokenStream {
    let encode = 
        // Encoding logic for primitive property
        match field_type {
            "u8" => 
                quote! {
                dest.put_u8(self.#field_name);
                },
            
            "u16" => quote!{
                dest.put_u16(self.#field_name);
            },
            "u32" => quote!{
                dest.put_u32(self.#field_name);
            },
            "i8" => quote!{
                dest.put_i8(self.#field_name);
            },
            "i16" => quote!{
                dest.put_i16(self.#field_name);
            },
            "i32" => quote!{
                dest.put_i32(self.#field_name);
            },
            "bool" => quote!{
                dest.put_u8(if self.#field_name { 1 } else { 0 });
            },
            _ => {
                compile_error2("Unsupported primitive type for MQTT codec encoding")
            }
        };
    encode
}

pub(crate) fn decode_for_primitive_property() -> proc_macro2::TokenStream {
    let decode = quote! {};

    decode
}
