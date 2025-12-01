use proc_macro::{self, TokenStream};
use quote::quote;
use syn::{parse_macro_input, ItemStruct};

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
    let packet_size_impl = quote! {};

    packet_size_impl.into()
}
