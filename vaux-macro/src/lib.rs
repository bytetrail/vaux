mod header;
mod size;

use crate::size::field_size;
use proc_macro::{self, TokenStream};
use quote::quote;
use syn::{parse_macro_input, ItemStruct};

#[proc_macro_attribute]
pub fn packet_header(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let property_codec_impl = derive_codec_property_size(item.clone());
    //let property_codec_impl = quote! {};
    //let property_codec_impl = TokenStream::from(property_codec_impl);
    let header_codec_impl = derive_encode_size(item.clone());

    // filter out the property attribute from the struct fields
    let input = parse_macro_input!(item as syn::ItemStruct);
    let struct_attrs = filter_attributes(&input.attrs, &["packet_header"]);
    let struct_name = &input.ident;
    // get the visibility of the struct
    let visibility = &input.vis;

    let mut struct_fields = quote! {};
    // process the fields
    match &input.fields {
        syn::Fields::Named(fields_named) => {
            for field in &fields_named.named {
                // filter out the property attribute
                let field_attrs = filter_attributes(&field.attrs, &["property"]);
                let field_vis = &field.vis;
                let field_name = field.ident.as_ref().expect("expected field name");
                let field_ty = &field.ty;

                struct_fields = quote! {
                    #struct_fields
                    #(#field_attrs)*
                    #field_vis #field_name: #field_ty,
                };
            }
        }
        _ => {
            return compile_error("packet_header can only be applied to structs with named fields");
        }
    };

    let expanded = quote! {
        #(#struct_attrs)*
        #visibility struct #struct_name {
            #struct_fields
        }
    };

    TokenStream::from(expanded)
        .into_iter()
        .chain(property_codec_impl)
        .chain(header_codec_impl)
        .collect()
}

#[proc_macro_derive(PropertyCodecSize, attributes(property))]
pub fn derive_codec_property_size(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ItemStruct);
    let name = &input.ident;

    // ensure that the struct has named fields
    let fields = match &input.fields {
        syn::Fields::Named(fields_named) => fields_named,
        _ => {
            return compile_error("Size can only be derived for structs with named fields");
        }
    };

    // Generate size calculation for each field
    let mut field_size_calculations = Vec::new();
    for field in &fields.named {
        if has_attribute(&field.attrs, "property") {
            let field_size_calc = field_size(field);
            field_size_calculations.push(field_size_calc);
            continue;
        }
    }

    let size_wrapper = quote! {
        impl crate::PropertyCodecSize for #name {
            fn property_size(&self) -> u32 {
                let mut property_size = 0;

                #(#field_size_calculations)*

                property_size
            }
        }
    };
    TokenStream::from(size_wrapper)
}

#[proc_macro_derive(HeaderCodecSize)]
pub fn derive_encode_size(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ItemStruct);
    let name = &input.ident;

    // ensure that the struct has named fields
    let fields = match &input.fields {
        syn::Fields::Named(fields_named) => fields_named,
        _ => {
            return compile_error("Size can only be derived for structs with named fields");
        }
    };
    // Generate size calculation for each field
    let mut field_size_calculations = Vec::new();
    for field in &fields.named {
        if !has_attribute(&field.attrs, "property") {
            let field_size_calc = field_size(field);
            field_size_calculations.push(field_size_calc);
            continue;
        }
    }

    let size_wrapper = quote! {
        impl crate::HeaderCodecSize for #name {
            fn header_size(&self) -> u32 {
                let mut total_size = 0;
                #(#field_size_calculations)*
                let property_size = self.property_size();
                total_size + property_size + crate::variable_byte_int_size(property_size)
            }
        }
    };
    TokenStream::from(size_wrapper)
}

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

/// Checks if the given attribute name exists in the list of attributes. This is used to
/// determine if a specific custom attribute is present on a struct or field. In the Size
/// macro, we might use this to check for attributes that specify a specific type for size
///  calculation.
fn has_attribute(attrs: &[syn::Attribute], name: &str) -> bool {
    attrs.iter().any(|attr| attr.path().is_ident(name))
}

/// Checks if any of the named fields has the specified attribute.
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

/// Filters out attributes whose path matches any in the exclude list. We use this
/// to remove our custom attributes before passing the struct to other macros.
fn filter_attributes(attrs: &Vec<syn::Attribute>, exclude: &[&str]) -> Vec<syn::Attribute> {
    attrs
        .iter()
        .filter(|attr| !exclude.iter().any(|r| attr.path().is_ident(r)))
        .cloned()
        .collect()
}
