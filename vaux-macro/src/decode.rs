use std::option;

use crate::{compile_error2, is_primitive_type};
use quote::quote;

const CODEC_ATTR: &str = "codec";
const PROPERTY_TYPE_ARG: &str = "property_type";
const DECODE_WITH_ARG: &str = "decode_with";

pub(crate) fn decode_field(field: &syn::Field) -> proc_macro2::TokenStream {
    let field_name = field.ident.as_ref().unwrap();
    let field_type = &field.ty;
    let is_property =
        crate::has_attribute_with_name_value(&field.attrs, CODEC_ATTR, PROPERTY_TYPE_ARG).unwrap();
    let decode_with =
        crate::attribute_with_name_value(&field.attrs, CODEC_ATTR, DECODE_WITH_ARG).unwrap();
    let property_type: Option<syn::Path> = crate::property_type(&field.attrs);
    if property_type.is_none() && is_property {
        return compile_error2("Property attribute requires a property_type argument");
    }
    // Generate the match arm prefix if this is a property
    let match_arm_prefix = if is_property {
        quote! {
            PropertyType::#property_type =>
        }
    } else {
        quote! {}
    };

    let stmt_separator = if is_property { "," } else { ";" };

    let field_decode = match field_type {
        syn::Type::Path(type_path) => {
            //let skip_if = get_skip_if_path(&field.attrs);
            let segment = &type_path.path.segments.last().unwrap().ident;
            let optional_field = match segment.to_string().as_str() {
                "Option" => true,
                _ => false,
            };
            // if decode_with.is_some() {
            //     decode_for_decode_with(
            //         &field_name,
            //         &decode_with.unwrap(),
            //         optional_field,
            //         property_type,
            //     )
            // } else {
            match segment.to_string().as_str() {
                "String" => decode_for_string(field_name, optional_field, property_type),
                "Option" => decode_for_option(&field_name, &type_path, property_type),
                "Vec" => decode_for_vec(&field_name, &type_path, optional_field, property_type),
                type_name => {
                    if is_primitive_type(type_name) {
                        decode_for_primitive(field_name, field_type, optional_field, property_type)
                    } else {
                        decode_for_type(field_name, field_type, is_property, property_type)
                    }
                }
            }
            //}
        }
        _ => {
            return compile_error2("Unsupported field type for Decode derive");
        }
    };

    quote! {
        #match_arm_prefix #field_decode
    }
}

fn decode_for_string(
    field_name: &syn::Ident,
    optional_field: bool,
    property_type: Option<syn::Path>,
) -> proc_macro2::TokenStream {
    if optional_field {
        quote! {
            let mut value = String::new()
            self.#field_name = Some(value.decode(src)?);

        }
    } else {
        quote! {
            self.#field_name.decode(src)?;
        }
    }
}

fn decode_for_option(
    field_name: &syn::Ident,
    type_path: &syn::TypePath,
    property_type: Option<syn::Path>,
) -> proc_macro2::TokenStream {
    let decode_stmt = match &type_path.path.segments.last().unwrap().arguments {
        syn::PathArguments::AngleBracketed(args) => match args.args.first().unwrap() {
            syn::GenericArgument::Type(syn::Type::Path(option_inner_type_path)) => {
                match option_inner_type_path
                    .path
                    .segments
                    .last()
                    .unwrap()
                    .ident
                    .to_string()
                    .as_str()
                {
                    "String" => {
                        quote! {
                            let mut value = String::new();
                            value.decode(src)?;
                            self.#field_name = Some(value);
                        }
                    }
                    type_name => {
                        if is_primitive_type(type_name) {
                            quote! {
                                let mut value = #option_inner_type_path::default();
                                value.decode(src)?;
                                self.#field_name = Some(value);
                            }
                        } else {
                            quote! {
                                let mut value = #option_inner_type_path::default();
                                value.decode(src)?;
                                self.#field_name = Some(value);
                            }
                        }
                    }
                }
            }
            _ => {
                return compile_error2("Unsupported Option inner type for Decode derive");
            }
        },
        _ => {
            return compile_error2("Unsupported Option inner type for Decode derive");
        }
    };

    decode_stmt
}

fn decode_for_primitive(
    field_name: &syn::Ident,
    field_type: &syn::Type,
    optional_field: bool,
    property_type: Option<syn::Path>,
) -> proc_macro2::TokenStream {
    if optional_field {
        quote! {
            let mut value = #field_type::default();
            self.#field_name = Some(value.decode(src)?);
        }
    } else {
        quote! {
            self.#field_name.decode(src)?;
        }
    }
}

fn decode_for_type(
    field_name: &syn::Ident,
    field_type: &syn::Type,
    is_property: bool,
    property_type: Option<syn::Path>,
) -> proc_macro2::TokenStream {
    if is_property {
        quote! {
            let mut value = #field_type::default();
            self.#field_name = Some(value.decode(src)?);
        }
    } else {
        quote! {
            self.#field_name.decode(src)?;
        }
    }
}

fn decode_for_vec(
    field_name: &syn::Ident,
    type_path: &syn::TypePath,
    optional_field: bool,
    property_type: Option<syn::Path>,
) -> proc_macro2::TokenStream {
    // if optional_field {
    //     quote! {
    //             self.#field_name = Some(codec::decode_binary_data(src)?)
    //     }
    // } else {
    //     quote! {
    //             self.#field_name = codec::decode_binary_data(src)?
    //     }
    // }

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
                                self.#field_name = Some(codec::decode_array_field(src)?)
                            }
                        } else {
                            quote! {
                                self.#field_name = codec::decode_array_field(src)?
                            }
                        }
                    }
                    "i8" | "u16" | "i16" | "u32" | "i32" | "u64" | "i64" | "f32" | "f64"
                    | "bool" => compile_error2(
                        "Decode derive does not support Vec of primitive types except Vec<u8>",
                    ),
                    type_name => {
                        // only support named types in the payload section of of packet
                        if optional_field {
                            quote! {}
                        } else {
                            quote! {}
                        }
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

    quote! {}
}
