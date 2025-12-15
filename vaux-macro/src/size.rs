use proc_macro::{self, TokenStream};
use quote::quote;
use syn::{parse_macro_input, ItemStruct};

pub(crate) fn field_size(field: &syn::Field) -> proc_macro2::TokenStream {
    let field_name = &field.ident;
    let field_type = &field.ty;
    let _attrs = &field.attrs;
    let field_calc = match field_type {
        syn::Type::Path(type_path) => {
            let segment = &type_path.path.segments.last().unwrap().ident;
            match segment.to_string().as_str() {
                "String" => size_for_string(field_name),
                "Option" => {
                    let generic_type = match &type_path.path.segments.last().unwrap().arguments {
                        syn::PathArguments::AngleBracketed(args) => args.args.first().unwrap(),
                        _ => {
                            return compile_error2("Unsupported Option type for Size derive");
                        }
                    };
                    match generic_type {
                        syn::GenericArgument::Type(syn::Type::Path(inner_type_path)) => {
                            let inner_segment =
                                &inner_type_path.path.segments.last().unwrap().ident;
                            match inner_segment.to_string().as_str() {
                                "String" => size_for_option_string(field_name),
                                type_name => {
                                    if is_primitive_type(type_name) {
                                        size_for_primitive_optional(field_name, type_name)
                                    } else {
                                        compile_error2(&format!(
                                            "Unsupported Option inner type '{}' for Size derive",
                                            type_name
                                        ))
                                    }
                                }
                            }
                        }
                        _ => compile_error2("Unsupported Option type for Size derive"),
                    }
                }
                _ => size_for_primitive(field_type),
            }
        }
        _ => compile_error2("Unsupported field type for Size derive"),
    };

    field_calc
}

fn size_for_option_string(field_name: &Option<syn::Ident>) -> proc_macro2::TokenStream {
    let field = field_name.as_ref().unwrap();
    quote! {
        if let Some(value) = &self.#field {
            total_size += 2; // MQTT string length prefix
            total_size += value.len() as u32;
        }
    }
}

fn size_for_string(field_name: &Option<syn::Ident>) -> proc_macro2::TokenStream {
    let field = field_name.as_ref().unwrap();
    quote! {
        total_size += 2; // MQTT string length prefix
        total_size += self.#field.len() as u32;
    }
}

fn size_for_vec_u8(field_name: &Option<syn::Ident>) -> proc_macro2::TokenStream {
    let field = field_name.as_ref().unwrap();
    quote! {
        total_size += 2; // MQTT binary length prefix
        total_size += self.#field.len() as u32;
    }
}

fn size_for_primitive_optional(
    field_name: &Option<syn::Ident>,
    field_type: &str,
) -> proc_macro2::TokenStream {
    let field = field_name.as_ref().unwrap();
    let size_calc = size_for_primitive_internal(field_type);
    quote! {
        if let Some(_) = &self.#field {
            #size_calc
        }
    }
}

fn size_for_primitive(field_type: &syn::Type) -> proc_macro2::TokenStream {
    // match the field type to determine if supported
    match field_type {
        syn::Type::Path(type_path) => {
            let segment = &type_path.path.segments.last().unwrap().ident;
            size_for_primitive_internal(segment.to_string().as_str())
        }
        _ => compile_error2("Unsupported field type for Size derive"),
    }
}

fn size_for_primitive_internal(field_type: &str) -> proc_macro2::TokenStream {
    match field_type {
        "u8" | "i8" | "bool" => {
            quote! {
                total_size += 1;
            }
        }
        "u16" | "i16" => {
            quote! {
                total_size += 2;
            }
        }
        "u32" | "i32" | "char" => {
            quote! {
                total_size += 4;
            }
        }
        _ => compile_error2(&format!(
            "Unsupported primitive type '{}' for Size derive",
            field_type
        )),
    }
}

// Helper for compile_error in quote context
fn compile_error2(message: &str) -> proc_macro2::TokenStream {
    let error_message = format!("Compile-time error in vaux-macro: {}", message);
    quote! {
        compile_error!(#error_message);
    }
}

/// Filters out attributes whose path matches any in the exclude list. We use this
/// to remove our custom attributes before passing the struct to other macros.
fn filter_attributes(attrs: &Vec<syn::Attribute>, exclude: &[&str]) -> Vec<syn::Attribute> {
    attrs
        .iter()
        .filter(|attr| !exclude.iter().any(|r| attr.path().is_ident(r)))
        .cloned()
        .collect()
}

/// Checks if the given attribute name exists in the list of attributes. This is used to
/// determine if a specific custom attribute is present on a struct or field. In the Size
/// macro, we might use this to check for attributes that specify a specific type for size
///  calculation.
fn has_attribute(attrs: &Vec<syn::Attribute>, name: &str) -> bool {
    attrs.iter().any(|attr| attr.path().is_ident(name))
}

/// Checks if the given type name is a supported primitive type for size calculation.
/// Currently supports u8, i8, bool, u16, i16, u32, i32, and char.
fn is_primitive_type(type_name: &str) -> bool {
    matches!(
        type_name,
        "u8" | "i8" | "bool" | "u16" | "i16" | "u32" | "i32" | "char"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_size_for_string() {
        let field_name = Some(syn::Ident::new(
            "test_string",
            proc_macro2::Span::call_site(),
        ));
        let tokens = size_for_string(&field_name);
        let expected = quote! {
            total_size += 2; // MQTT string length prefix
            total_size += self.test_string.len() as u32;
        };
        assert_eq!(tokens.to_string(), expected.to_string());
    }

    #[test]
    fn test_size_for_option_string() {
        let field_name = Some(syn::Ident::new(
            "opt_string",
            proc_macro2::Span::call_site(),
        ));
        let tokens = size_for_option_string(&field_name);
        let expected = quote! {
            if let Some(value) = &self.opt_string {
                total_size += 2; // MQTT string length prefix
                total_size += value.len() as u32;
            }
        };
        assert_eq!(tokens.to_string(), expected.to_string());
    }

    #[test]
    fn test_size_for_vec_u8() {
        let field_name = Some(syn::Ident::new("data", proc_macro2::Span::call_site()));
        let tokens = size_for_vec_u8(&field_name);
        let expected = quote! {
            total_size += 2; // MQTT binary length prefix
            total_size += self.data.len() as u32;
        };
        assert_eq!(tokens.to_string(), expected.to_string());
    }

    #[test]
    fn test_size_for_u8_primitive() {
        // u8
        let field_name = Some(syn::Ident::new("test_u8", proc_macro2::Span::call_site()));
        let field_type = syn::parse_str::<syn::Type>("u8").unwrap();
        let tokens = size_for_primitive(&field_type);
        let expected = quote! {
            total_size += 1;
        };
        assert_eq!(tokens.to_string(), expected.to_string());
    }

    #[test]
    fn test_size_for_u16_primitive() {
        // u16
        let field_name = Some(syn::Ident::new("test_u16", proc_macro2::Span::call_site()));
        let field_type = syn::parse_str::<syn::Type>("u16").unwrap();
        let tokens = size_for_primitive(&field_type);
        let expected = quote! {
            total_size += 2;
        };
        assert_eq!(tokens.to_string(), expected.to_string());
    }

    #[test]
    fn test_size_for_u32_primitive() {
        // u32
        let field_name = Some(syn::Ident::new("test_u32", proc_macro2::Span::call_site()));
        let field_type = syn::parse_str::<syn::Type>("u32").unwrap();
        let tokens = size_for_primitive(&field_type);
        let expected = quote! {
            total_size += 4;
        };
        assert_eq!(tokens.to_string(), expected.to_string());
    }
}
