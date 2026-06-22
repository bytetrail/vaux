mod decode;
mod encode;
mod size;
mod util;
use crate::decode::decode_internal;
use crate::encode::{encode_internal, property_encode_internal};
use crate::size::field_size;
use crate::util::{
    abbreviated_when_expr, compile_error, filter_attributes, get_packet_type, get_skip_if_path,
    has_attribute_with_name_value, is_payload_field, is_property_field, skip_field,
    struct_has_attribute_with_name_value,
};
use proc_macro::{self, TokenStream};
use quote::quote;
use syn::{parse_macro_input, ItemStruct};

pub(crate) const CODEC_ATTR: &str = "codec";
pub(crate) const PACKET_ATTR: &str = "packet";
pub(crate) const PACKET_ATTR_PACKET_TYPE_ARG: &str = "packet_type";
pub(crate) const CODEC_ATTR_PROPERTY_TYPE_ARG: &str = "property_type";
pub(crate) const CODEC_ATTR_DECODE_WITH_ARG: &str = "decode_with";
pub(crate) const CODEC_ATTR_ENCODE_WITH_ARG: &str = "encode_with";
pub(crate) const CODEC_ATTR_SKIP_ARG: &str = "skip";
pub(crate) const CODEC_ATTR_SKIP_IF_ARG: &str = "skip_if";
pub(crate) const CODEC_ATTR_PAYLOAD_ARG: &str = "payload_type";
pub(crate) const CODEC_ATTR_CODEC_MIN_SIZE_ARG: &str = "min_decode";
pub(crate) const CODEC_ATTR_PAYLOAD_TYPE_REMAINING: &str = "remaining";
pub(crate) const CODEC_ATTR_PAYLOAD_TYPE_FIELD: &str = "field";
pub(crate) const CODEC_ATTR_ABBREVIATED_WHEN_ARG: &str = "abbreviated_when";
pub(crate) const CODEC_ATTR_NON_ABBREVIATED_ARG: &str = "non_abbreviated";
pub(crate) const CODEC_ATTR_AS_PACKET_ARG: &str = "as_packet";
pub(crate) const CODEC_ATTR_SKIP_DECODE_ARG: &str = "skip_decode";

#[proc_macro_attribute]
pub fn packet(args: TokenStream, input: TokenStream) -> TokenStream {
    let codec_size_impl = derive_codec_size(input.clone());
    let codec_property_size_impl = derive_codec_property_size(input.clone());
    let property_encode_impl = derive_property_encode(input.clone());
    let encode_impl = encode_packet(input.clone());

    let pre_parse: syn::ItemStruct = syn::parse(input.clone()).expect("failed to parse struct");
    let skip_decode = crate::util::is_skip_decode(&pre_parse.attrs);
    let decode_impl = if skip_decode {
        TokenStream::new()
    } else {
        derive_decode(input.clone())
    };

    // get the packet type from the control packet attribute
    let input = parse_macro_input!(input as syn::ItemStruct);
    let struct_attrs = filter_attributes(&input.attrs, &[PACKET_ATTR, CODEC_ATTR]);
    let struct_name = &input.ident;
    let struct_vis = &input.vis;

    // get the packet type from the packet attr args
    let nv = parse_macro_input!(args as syn::MetaNameValue);
    let packet_type = get_packet_type(&nv);

    // check if the user's derive list includes Default
    let has_default_derive = struct_attrs.iter().any(|attr| {
        if attr.path().is_ident("derive") {
            if let Ok(nested) = attr.parse_args_with(syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated) {
                return nested.iter().any(|p| p.is_ident("Default"));
            }
        }
        false
    });

    // filter Default from derive attributes to replace with a packet-aware impl
    let struct_attrs: Vec<syn::Attribute> = if has_default_derive {
        struct_attrs.iter().map(|attr| {
            if attr.path().is_ident("derive") {
                if let Ok(nested) = attr.parse_args_with(syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated) {
                    let filtered: Vec<&syn::Path> = nested.iter().filter(|p| !p.is_ident("Default")).collect();
                    if filtered.is_empty() {
                        return syn::parse_quote!(#[cfg(any())]);
                    }
                    return syn::parse_quote!(#[derive(#(#filtered),*)]);
                }
            }
            attr.clone()
        }).collect()
    } else {
        struct_attrs
    };

    // filter out the codec attributes as no longer needed and add the fixed header
    let mut struct_fields = quote! {
        pub fixed_header: codec::FixedHeader,
    };
    let mut default_field_inits = Vec::new();

    match &input.fields {
        syn::Fields::Named(fields_named) => {
            for field in &fields_named.named {
                let field_attrs = filter_attributes(&field.attrs, &[CODEC_ATTR]);
                let field_vis = &field.vis;
                let field_type = &field.ty;
                let field_name = &field.ident;

                struct_fields = quote! {
                    #struct_fields

                    #(#field_attrs)*
                    #field_vis #field_name: #field_type,
                };
                if has_default_derive {
                    default_field_inits.push(quote! {
                        #field_name: Default::default(),
                    });
                }
            }
        }
        syn::Fields::Unnamed(_) => {
            return compile_error(
                "packet attribute cannot be applied to tuple structs",
            )
        }
        syn::Fields::Unit => {
            struct_fields = quote! {
                #struct_fields
            }
        }
    }

    let default_impl = if has_default_derive {
        quote! {
            impl Default for #struct_name {
                fn default() -> Self {
                    Self {
                        fixed_header: codec::FixedHeader::new(#packet_type),
                        #(#default_field_inits)*
                    }
                }
            }
        }
    } else {
        quote! {}
    };

    let output = quote! {
        #(#struct_attrs)*
        #struct_vis struct #struct_name {
            #struct_fields
        }

        #default_impl

        impl #struct_name {
            #[allow(clippy::needless_update)]
            #struct_vis fn new_with_fixed_header(fixed_header: codec::FixedHeader) -> Result<Self, codec::MqttCodecError> {
                if fixed_header.packet_type != #packet_type {
                    return Err( MqttCodecError::new(
                        format!("Unsuppprted PacketType for {}", "#struct_name").as_str()));
                }
                Ok(Self {
                    fixed_header,
                    ..Default::default()
                })
            }
        }
    };

    TokenStream::from(output)
        .into_iter()
        .chain(codec_size_impl)
        .chain(codec_property_size_impl)
        .chain(property_encode_impl)
        .chain(encode_impl)
        .chain(decode_impl)
        .collect()
}

#[proc_macro_derive(PropertyCodecSize, attributes(codec))]
pub fn derive_codec_property_size(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ItemStruct);
    let name = &input.ident;
    // ensure that the struct does not have tuple fields
    let fields = match &input.fields {
        syn::Fields::Named(fields_named) => fields_named,
        syn::Fields::Unnamed(_) => {
            return compile_error("Size cannot be used for tuple structs");
        }
        syn::Fields::Unit => {
            // return an empty token stream for unit structs
            return TokenStream::new();
        }
    };

    // Generate size calculation for each field
    let mut field_size_calculations = Vec::new();
    for field in &fields.named {
        if let Ok(true) =
            has_attribute_with_name_value(&field.attrs, CODEC_ATTR, CODEC_ATTR_PROPERTY_TYPE_ARG)
        {
            if let Some(skip_path) = get_skip_if_path(&field.attrs) {
                let field_name = field.ident.as_ref().unwrap();
                let field_size_calc = field_size(field);
                field_size_calculations.push(quote! {
                    if !#skip_path(&self.#field_name) {
                        #field_size_calc
                    }
                });
                continue;
            } else {
                let field_size_calc = field_size(field);
                field_size_calculations.push(field_size_calc);
                continue;
            }
        }
    }

    let size_wrapper = quote! {
        impl codec::PropertyCodecSize for #name {
            fn property_size(&self) -> u32 {
                use codec::CodecSize;

                let mut property_size = 0;
                #(#field_size_calculations)*
                property_size
            }
        }
    };
    TokenStream::from(size_wrapper)
}



/// Derives the CodecSize trait for a struct, calculating the total size based on its fields.
/// If the struct contains fields marked as properties, it includes the property size and
/// the variable byte integer size for the property length. The generated implementation
/// iterates over each field, calculating its size according to its type and attributes.
///
/// # Arguments
/// * `input` - A TokenStream representing the struct to derive CodecSize for.
///
/// #Attributes
/// * `codec` - Custom attribute used to specify property types for fields.
///
/// # Returns
/// TokenStream representing the generated implementation.
///
/// <em>Note:</em> This macro does not support tuple structs and relies on the
/// 'vaux_mqtt::codec' module for size calculation functions.
#[proc_macro_derive(CodecSize, attributes(codec))]
pub fn derive_codec_size(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ItemStruct);
    let name = &input.ident;

    // ensure that the struct does not have tuple fields
    let fields = match &input.fields {
        syn::Fields::Named(fields_named) => Some(fields_named),
        syn::Fields::Unnamed(_) => {
            return compile_error("Size macro does not support tuple structs");
        }
        syn::Fields::Unit => None,
        
    };
    let has_properties = if let Some(fields) = fields {
        struct_has_attribute_with_name_value(fields, CODEC_ATTR, CODEC_ATTR_PROPERTY_TYPE_ARG)
    } else {
        false
    };
    let mut field_sizes = Vec::new();
    let mut non_abbreviated_field_sizes = Vec::new();

    if let Some(fields) = fields {
        for field in &fields.named {
            if has_attribute_with_name_value(&field.attrs, CODEC_ATTR, CODEC_ATTR_PROPERTY_TYPE_ARG)
                .unwrap_or(false)
            {
                continue;
            }
            let is_non_abbrev = crate::util::is_non_abbreviated_field(&field.attrs).unwrap_or(false);
            let field_size_calc = field_size(field);
            let sized = if let Some(skip_path) = get_skip_if_path(&field.attrs) {
                let field_name = field.ident.as_ref().unwrap();
                quote! {
                    if !#skip_path(&self.#field_name) {
                        #field_size_calc
                    }
                }
            } else {
                field_size_calc
            };
            if is_non_abbrev {
                non_abbreviated_field_sizes.push(sized);
            } else {
                field_sizes.push(sized);
            }
        }
    }
    let abbreviated_when = abbreviated_when_expr(&input.attrs);

    let size_wrapper = if has_properties {
        if let Some(abbreviated_expr) = abbreviated_when {
            quote! {
                impl codec::CodecSize for #name {
                    fn codec_size(&self) -> u32 {
                        use codec::PropertyCodecSize;
                        let mut total_size = 0;
                        #(#field_sizes)*
                        if #abbreviated_expr {
                            return total_size;
                        }
                        #(#non_abbreviated_field_sizes)*
                        let property_size = self.property_size();
                        total_size + property_size + codec::variable_byte_int_size(property_size)
                    }
                }
            }
        } else {
            quote! {
                impl codec::CodecSize for #name {
                    fn codec_size(&self) -> u32 {
                        use codec::PropertyCodecSize;
                        let mut total_size = 0;
                        #(#field_sizes)*
                        #(#non_abbreviated_field_sizes)*
                        let property_size = self.property_size();
                        total_size + property_size + codec::variable_byte_int_size(property_size)
                    }
                }
            }
        }
    } else {
        quote! {
            impl codec::CodecSize for #name {
                fn codec_size(&self) -> u32 {
                    let mut total_size = 0;
                    #(#field_sizes)*
                    #(#non_abbreviated_field_sizes)*
                    total_size
                }
            }
        }
    };

    TokenStream::from(size_wrapper)
}

#[proc_macro_derive(PropertyEncode, attributes(codec))]
pub fn derive_property_encode(input: TokenStream) -> TokenStream {
    property_encode_internal(input, false)
}

#[proc_macro_derive(Encode, attributes(codec))]
pub fn derive_encode(input: TokenStream) -> TokenStream {
    encode_internal(input, false)
}

fn encode_packet(input: TokenStream) -> TokenStream {
    encode_internal(input, true)
}

#[proc_macro_derive(Decode, attributes(codec_as))]
pub fn derive_decode(input: TokenStream) -> TokenStream {
    decode_internal(input)
}


