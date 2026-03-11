use proc_macro::TokenStream;
use quote::quote;
use syn::{punctuated::Punctuated, Meta, Token};

use crate::{
    CODEC_ATTR, CODEC_ATTR_CODEC_MIN_SIZE_ARG, CODEC_ATTR_PAYLOAD_ARG, CODEC_ATTR_PROPERTY_TYPE_ARG, CODEC_ATTR_SKIP_ARG, CODEC_ATTR_SKIP_IF_ARG, PACKET_ATTR_PACKET_TYPE_ARG
};

/// Generates a compile-time error with the given message.
pub(crate) fn compile_error(message: &str) -> TokenStream {
    let error_message = format!("Compile-time error in vaux-macro: {}", message);
    TokenStream::from(quote! {
        compile_error!(#error_message);
    })
}

pub(crate) fn compile_error2(message: &str) -> proc_macro2::TokenStream {
    let error_message = format!("Compile-time error in vaux-macro: {}", message);
    quote! {
        compile_error!(#error_message);
    }
}

pub(crate) fn skip_field(attrs: &[syn::Attribute]) -> Result<bool, syn::Error> {
    return has_attribute_with_path(attrs, CODEC_ATTR, CODEC_ATTR_SKIP_ARG);
}

pub(crate) fn is_property_field(attrs: &[syn::Attribute]) -> Result<bool, syn::Error> {
    has_attribute_with_name_value(attrs, CODEC_ATTR, CODEC_ATTR_PROPERTY_TYPE_ARG)
}

pub(crate) fn is_payload_field(attrs: &[syn::Attribute]) -> Result<bool, syn::Error> {
    has_attribute_with_name_value(attrs, CODEC_ATTR, CODEC_ATTR_PAYLOAD_ARG)
}

/// Returns the payload type if there is a ```payload_type``` in the attributes for the field.
/// The payload_type attribute is used to specify a different type for the field during
/// encoding/decoding/size calculation. This function extracts the type specified in the
/// payload_type attribute.
pub(crate) fn payload_type(attrs: &[syn::Attribute]) -> Option<String> {
    attrs.iter().find_map(|attr| {
        if attr.path().is_ident(CODEC_ATTR) {
            let nested = attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated);
            nested.unwrap().iter().find_map(|meta| {
                if meta.path().is_ident(CODEC_ATTR_PAYLOAD_ARG) {
                    if let Meta::NameValue(nv_pair) = meta {
                        if let syn::Expr::Lit(lit_str) = &nv_pair.value {
                            if let syn::Lit::Str(lit_str) = &lit_str.lit {
                                Some(lit_str.value())
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
        } else {
            None
        }
    })
}

pub(crate) fn min_decode_size(attrs: &[syn::Attribute]) -> Option<usize> {
    attrs.iter().find_map(|attr| {
        if attr.path().is_ident(CODEC_ATTR) {
            let nested = attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated);
            nested.unwrap().iter().find_map(|meta| {
                if meta.path().is_ident(CODEC_ATTR_CODEC_MIN_SIZE_ARG) {
                    if let Meta::NameValue(nv_pair) = meta {
                        if let syn::Expr::Lit(lit_int) = &nv_pair.value {
                            if let syn::Lit::Int(lit_int) = &lit_int.lit {
                                Some(lit_int.base10_parse::<usize>().unwrap())
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
        } else {
            None
        }
    })
}

/// Returns the skip_if path if there is a ```skip_if``` in the attributes for the field.
/// The skip_if attribute is used to conditionally skip encoding/decoding/size calculation
/// of a field based on a specific condition. This function extracts the condition specified
/// in the skip_if attribute.
///
/// The path must resolve to  function that is called with &self and returns a bool indicating
/// whether to skip the field or not.
///
pub(crate) fn get_skip_if_path(attrs: &[syn::Attribute]) -> Option<syn::Path> {
    let codec_attrs = attrs
        .iter()
        .filter(|attr| attr.path().is_ident(CODEC_ATTR))
        .collect::<Vec<_>>();
    for attr in &codec_attrs {
        let nested = attr
            .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
            .ok()?;
        for meta in nested {
            if let Meta::NameValue(nv_pair) = meta {
                if nv_pair.path.is_ident(CODEC_ATTR_SKIP_IF_ARG) {
                    if let syn::Expr::Lit(lit_expr) = &nv_pair.value {
                        if let syn::Lit::Str(lit_str) = &lit_expr.lit {
                            let path: syn::Path = lit_str.parse().unwrap();
                            return Some(path);
                        }
                    }
                }
            }
        }
    }
    None
}

pub(crate) fn get_packet_type(nv: &syn::MetaNameValue) -> Option<syn::Path> {
    if nv.path.is_ident(PACKET_ATTR_PACKET_TYPE_ARG) {
        if let syn::Expr::Lit(lit_expr) = &nv.value {
            if let syn::Lit::Str(lit_str) = &lit_expr.lit {
                let path: syn::Path = lit_str.parse().unwrap();
                return Some(path);
            }
        }
    }
    None
}

/// Checks if any of the named fields has the specified attribute.
#[allow(dead_code)]
pub(crate) fn struct_has_attribute(fields: &syn::FieldsNamed, attribute_name: &str) -> bool {
    for field in &fields.named {
        for attr in &field.attrs {
            if attr.path().is_ident(attribute_name) {
                return true;
            }
        }
    }
    false
}

/// Checks if the given attribute name exists in the list of attributes. This is used to
/// determine if a specific custom attribute is present on a struct or field. In the Size
/// macro, we might use this to check for attributes that specify a specific type for size
///  calculation.
#[allow(dead_code)]
fn has_attribute(attrs: &[syn::Attribute], name: &str) -> bool {
    attrs.iter().any(|attr| attr.path().is_ident(name))
}

pub(crate) fn has_attribute_with_path(
    attrs: &[syn::Attribute],
    name: &str,
    value: &str,
) -> Result<bool, syn::Error> {
    let codec_attrs = attrs
        .iter()
        .filter(|attr| attr.path().is_ident(name))
        .collect::<Vec<_>>();
    for attr in &codec_attrs {
        let nested = attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?;
        for meta in nested {
            if let Meta::Path(p) = meta {
                if p.is_ident(value) {
                    return Ok(true);
                }
            } else {
                continue;
            }
        }
    }
    Ok(false)
}

/// Checks if the given attribute with a specific name-value pair exists in the list of attributes.
/// This is useful for attributes that have key-value pairs, allowing us to check for specific
/// configurations within an attribute. The codec attribute often contains such key-value pairs to
/// customize behavior for encoding, decoding, or size calculation.
pub(crate) fn has_attribute_with_name_value(
    attrs: &[syn::Attribute],
    name: &str,
    key: &str,
) -> Result<bool, syn::Error> {
    let codec_attrs = attrs
        .iter()
        .filter(|attr| attr.path().is_ident(name))
        .collect::<Vec<_>>();
    for attr in &codec_attrs {
        let nested = attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?;
        for meta in nested {
            if let Meta::NameValue(nv_pair) = meta {
                if nv_pair.path.is_ident(key) {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

/// Retrieves the attribute with the specified name-value pair from the list of attributes.
/// This is useful for attributes that have key-value pairs, allowing us to check for specific
/// configurations within an attribute. The codec attribute often contains such key-value pairs to
/// customize behavior for encoding, decoding, or size calculation.
pub(crate) fn attribute_with_name_value<'a>(
    attrs: &'a [syn::Attribute],
    name: &str,
    key: &str,
) -> Result<Option<&'a syn::Attribute>, syn::Error> {
    let codec_attrs = attrs
        .iter()
        .filter(|attr| attr.path().is_ident(name))
        .collect::<Vec<_>>();
    for attr in &codec_attrs {
        let nested = attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?;
        for meta in nested {
            if let Meta::NameValue(nv_pair) = meta {
                if nv_pair.path.is_ident(key) {
                    return Ok(Some(attr));
                }
            }
        }
    }
    Ok(None)
}

pub(crate) fn struct_has_attribute_with_name_value(
    fields: &syn::FieldsNamed,
    attribute_name: &str,
    key: &str,
) -> bool {
    for field in &fields.named {
        for attr in &field.attrs {
            if attr.path().is_ident(attribute_name) {
                if let Ok(true) = has_attribute_with_name_value(&field.attrs, attribute_name, key) {
                    return true;
                }
            }
        }
    }
    false
}

/// Checks if the given type name is a supported primitive type for size calculation.
/// Currently supports u8, i8, bool, u16, i16, u32, i32, and char.
pub(crate) fn is_primitive_type(type_name: &str) -> bool {
    matches!(
        type_name,
        "u8" | "i8"
            | "u16"
            | "i16"
            | "u32"
            | "i32"
            | "bool"
            | "u64"
            | "i64"
            | "f32"
            | "f64"
            | "char"
    )
}

/// Retrieves the property type path from the attributes if it exists.
pub(crate) fn property_type(attrs: &[syn::Attribute]) -> Option<syn::Path> {
    attrs.iter().find_map(|attr| {
        if attr.path().is_ident(CODEC_ATTR) {
            let nested = attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated);
            nested.unwrap().iter().find_map(|meta| {
                if meta.path().is_ident(CODEC_ATTR_PROPERTY_TYPE_ARG) {
                    if let Meta::NameValue(nv_pair) = meta {
                        if let syn::Expr::Lit(lit_str) = &nv_pair.value {
                            if let syn::Lit::Str(lit_str) = &lit_str.lit {
                                Some(lit_str.parse().unwrap())
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
        } else {
            None
        }
    })
}



/// Filters out attributes whose path matches any in the exclude list. We use this
/// to remove our custom attributes before passing the struct to other macros.
pub(crate) fn filter_attributes(
    attrs: &Vec<syn::Attribute>,
    exclude: &[&str],
) -> Vec<syn::Attribute> {
    attrs
        .iter()
        .filter(|attr| !exclude.iter().any(|r| attr.path().is_ident(r)))
        .cloned()
        .collect()
}
