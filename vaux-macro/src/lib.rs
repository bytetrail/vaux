mod header;
mod size;

use proc_macro::{self, TokenStream};
use quote::quote;
use syn::{parse_macro_input, ItemStruct};

use crate::size::field_size;

#[proc_macro_attribute]
pub fn payload_size(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemStruct);
    let struct_name = &input.ident;
    let payload_size = attr
        .into_iter()
        .next()
        .expect("Expected a payload size argument");

    let payload_size_str = match payload_size {
        proc_macro::TokenTree::Literal(lit) => lit.to_string(),
        _ => panic!("Expected a literal for payload size"),
    };

    let payload_size_value: u32 = payload_size_str
        .parse()
        .expect("Invalid payload size value");
    let payload_size_impl = quote! {

        impl PayloadSize for #struct_name {
             fn size(&self) -> u32 {
                 #payload_size_value
             }
        }
    };

    let expanded = quote! {
        #input

        #payload_size_impl
    };

    TokenStream::from(expanded)
}

#[proc_macro_attribute]
pub fn header_size(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemStruct);
    let struct_name = &input.ident;
    let header_size = attr
        .into_iter()
        .next()
        .expect("Expected a header size argument");
    let header_size_str = match header_size {
        proc_macro::TokenTree::Literal(lit) => lit.to_string(),
        _ => panic!("Expected a literal for header size"),
    };
    let header_size_value: u32 = header_size_str.parse().expect("Invalid header size value");
    let header_size_impl = quote! {

        impl HeaderSize for #struct_name {
             fn header_size(&self) -> u32 {
                 #header_size_value
             }
        }

    };

    let expanded = quote! {
        #input

        #header_size_impl
    };

    TokenStream::from(expanded)
}

#[proc_macro_derive(PacketProperties)]
pub fn packet_properties_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ItemStruct);
    let struct_name = &input.ident;

    let packet_properties_impl = quote! {
        impl PacketProperties for #struct_name {
            fn properties(&self) -> &PropertyBundle {
                &self.props
            }

            fn properties_mut(&mut self) -> &mut PropertyBundle {
                &mut self.props
            }

            fn set_properties(&mut self, props: PropertyBundle) {
                self.props = props;
            }
        }
    };
    packet_properties_impl.into()
}

#[proc_macro_derive(PacketSize)]
pub fn packet_size_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ItemStruct);
    let struct_name = &input.ident;

    let packet_size_impl = quote! {
        impl PacketSize for #struct_name {
            fn packet_size(&self) -> u32 {
                let property_remaining = self.property_size();
                let len = variable_byte_int_size(property_remaining);
                self.header_size() + len + property_remaining + self.payload_size()
            }

            fn property_size(&self) -> u32 {
                self.props.size()
            }

            fn payload_size(&self) -> u32 {
                PayloadSize::size(self)
            }
        }
    };

    packet_size_impl.into()
}

#[proc_macro_attribute]
pub fn PacketHeader(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as syn::ItemStruct);
    let struct_name = &input.ident;
    // get the visibility of the struct
    let visibility = &input.vis;
    // parse the attribute to get the packet type
    let packet_type: syn::LitStr = parse_macro_input!(attr as syn::LitStr);
    let packet_type_str = packet_type.value();
    // generate the new struct name
    let struct_name_str = format!("{}Header", struct_name);
    let struct_name_ident = syn::Ident::new(&struct_name_str, struct_name.span());

    let gen = quote! {
        #input

        #visibility type #struct_name_ident = crate::header::VariableHeader<#struct_name>;
    };
    gen.into()
}

#[proc_macro_derive(Size)]
pub fn derive_size(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ItemStruct);
    let name = &input.ident;

    // ensure that the struct has named fields
    let fields = match &input.fields {
        syn::Fields::Named(fields_named) => &fields_named.named,
        _ => {
            return compile_error("Size can only be derived for structs with named fields");
        }
    };

    // Generate size calculation for each field
    let mut field_size_calculations = Vec::new();
    for field in fields {
        let field_size_calc = field_size(field);
        field_size_calculations.push(field_size_calc);
    }

    let expanded = quote! {
        impl crate::Size for #name {
            fn size(&self) -> u32 {
                let mut total_size = 0;

                #(#field_size_calculations)*

                total_size
            }
        }
    };
    TokenStream::from(expanded)
}

/// Generates a compile-time error with the given message.
pub(crate) fn compile_error(message: &str) -> TokenStream {
    let error_message = format!("Compile-time error in vaux-macro: {}", message);
    TokenStream::from(quote! {
        compile_error!(#error_message);
    })
}
