// Copyright (c) Sean Lawlor
//
// This source code is licensed under both the MIT license found in the
// LICENSE-MIT file in the root directory of this source tree.

//! Procedure macro for Message formatting in `ractor_cluster`. This implements
//! the `ractor::Message` trait for non-serializable and serializable messages automatically.
//!
//! note: rampant use of `cargo expand` was used in the making of this macro. E.g.
//!
//! ```text
//! cargo expand -p ractor_playground 2>&1 > expand.tmp.rs
//! ```
//!
//! Caveats:
//!
//! 1. Non-serializable macros are simply getting `impl ractor::Message for MyStructOrEnum` added onto their struct
//! 2. Serializable messages have to have a few formatting requirements.
//!    a. All variants of the enum will be numbered based on their lexicographical ordering, which is sent over-the-wire in order to decode which
//!    variant was called. This is the `index` field on any variant of `ractor::message::SerializedMessage`
//!    b. All properties of the message **MUST** implement the `ractor::BytesConvertable` trait which means they supply a `to_bytes` and `from_bytes` method. Many
//!    types are pre-done for you in `ractor`'s definition of the trait
//!    c. For RPCs, the LAST argument **must** be the reply channel. Additionally the type of message the channel is expecting back must also implement `ractor::BytesConvertable`
//!    d. Lastly, for RPCs, they should additionally be decorated with `#[rpc]` on each variant's definition. This helps the macro identify that it
//!    is an RPC and will need port handler

extern crate proc_macro;

use std::borrow::Cow;

use proc_macro::TokenStream;
use quote::format_ident;
use quote::quote;
use quote::ToTokens;
use syn::spanned::Spanned;
use syn::AngleBracketedGenericArguments;
use syn::DeriveInput;
use syn::Fields;
use syn::Ident;
use syn::TypePath;
use syn::Variant;
use syn::{self};

/// Parse the `#[fields(...)]` attribute from a variant, returning field names if present.
///
/// Returns `None` if the attribute is not present, or `Some(Vec<String>)` with the field names.
/// The number of names should match the number of tuple fields (excluding RpcReplyPort for RPCs).
fn parse_fields_attribute(variant: &Variant) -> syn::Result<Option<Vec<String>>> {
    for attr in &variant.attrs {
        if attr.path().is_ident("fields") {
            let mut field_names = Vec::new();

            attr.parse_nested_meta(|meta| {
                if let Some(ident) = meta.path.get_ident() {
                    field_names.push(ident.to_string());
                    Ok(())
                } else {
                    Err(meta.error("expected identifier"))
                }
            })?;

            if field_names.is_empty() {
                return Err(syn::Error::new_spanned(
                    attr,
                    "#[fields(...)] requires at least one field name",
                ));
            }

            return Ok(Some(field_names));
        }
    }
    Ok(None)
}

/// Get the field key for a given index, using named fields if available.
///
/// For unnamed (tuple) fields, this returns either the ordinal index as a string ("0", "1", etc.)
/// or the corresponding name from `#[fields(...)]` if provided.
///
/// Returns a `Cow<str>` to avoid unnecessary allocations when borrowing from the field names vector.
fn get_field_key<'a>(index: usize, field_names: &'a Option<Vec<String>>) -> Cow<'a, str> {
    if let Some(names) = field_names {
        if index < names.len() {
            return Cow::Borrowed(&names[index]);
        }
    }
    Cow::Owned(index.to_string())
}

/// Derive `ractor::Message` for messages that are local-only
#[proc_macro_derive(RactorMessage)]
pub fn ractor_message_derive_macro(input: TokenStream) -> TokenStream {
    // Construct a representation of Rust code as a syntax tree
    // that we can manipulate
    let ast: DeriveInput = match syn::parse(input) {
        Ok(ast) => ast,
        Err(err) => return err.to_compile_error().into(),
    };

    let name = &ast.ident;
    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();
    let gen = quote! {
        impl #impl_generics ractor::Message for #name #ty_generics #where_clause {}
    };
    gen.into()
}

/// Derive `ractor::Message` for messages that can be sent over the network
///
/// Serializable messages have to have a few formatting requirements.
/// 1. All variants of the enum will be tagged based on their variant name, which is sent over-the-wire in order to decode which
///    variant was called. This is the `variant` field on `ractor::message::SerializedMessage::Cast` and `Call`.
/// 2. All properties of the message **MUST** implement the `ractor::BytesConvertable` trait which means they supply a `to_bytes` and `from_bytes` method. Many
///    types are pre-done for you in `ractor`'s definition of the trait
/// 3. For RPCs, the LAST argument **must** be the reply channel. Additionally the type of message the channel is expecting back must also implement `ractor::BytesConvertable`
/// 4. Lastly, for RPCs, they should additionally be decorated with `#[rpc]` on each variant's definition. This helps the macro identify that it
///    is an RPC and will need port handler
/// 5. For backwards compatibility, you can add new variants as long as you don't rename variants until all nodes in the cluster are upgraded.
///
/// ## Shell Introspection
///
/// Optionally, you can add the `#[ractor_shell]` attribute to enable shell introspection.
/// This generates an implementation of `ractor::SchemaProvider` (when the `shell-introspection`
/// feature is enabled) that allows the ractor_shell to:
///
/// - Query the message schema to show available variants and their fields
/// - Convert JSON input to properly-typed messages
/// - Send messages through the normal ractor_cluster path (enabling tracing)
///
/// Example:
/// ```ignore
/// #[derive(RactorClusterMessage)]
/// #[ractor_shell]
/// pub enum MyMessage {
///     Ping,
///     SetValue(i32),
///     #[rpc]
///     GetValue(RpcReplyPort<i32>),
/// }
/// ```
///
/// ## Named Fields with `#[fields(...)]`
///
/// By default, tuple-style enum variants use ordinal positions ("0", "1", etc.) as field
/// keys in JSON schema and serialization. You can provide human-readable field names using
/// the `#[fields(...)]` attribute:
///
/// ```ignore
/// #[derive(RactorClusterMessage)]
/// #[ractor_shell]
/// pub enum MyMessage {
///     /// Without #[fields]: use {"0": "hello", "1": 42}
///     Greeting(String, i32),
///
///     /// With #[fields]: use {"message": "hello", "count": 42}
///     #[fields(message, count)]
///     GreetingNamed(String, i32),
///
///     /// For RPCs, only name the non-reply fields
///     #[rpc]
///     #[fields(peer_name)]
///     IsPeer(String, RpcReplyPort<bool>),
/// }
/// ```
///
/// The `#[fields(...)]` attribute is optional. When omitted, ordinal positions are used.
/// Field names improve the shell/JSON interface without affecting the binary wire protocol.
#[proc_macro_derive(RactorClusterMessage, attributes(rpc, ractor_shell, fields))]
pub fn ractor_cluster_message_derive_macro(input: TokenStream) -> TokenStream {
    // Construct a representation of Rust code as a syntax tree
    // that we can manipulate
    let ast: DeriveInput = match syn::parse(input) {
        Ok(ast) => ast,
        Err(err) => return err.to_compile_error().into(),
    };

    // Build the trait implementation
    match impl_message_macro(&ast) {
        Ok(tokens) => tokens,
        Err(err) => err.to_compile_error().into(),
    }
}

fn impl_message_macro(ast: &syn::DeriveInput) -> syn::Result<TokenStream> {
    let name = &ast.ident;
    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();

    // Check if the #[ractor_shell] attribute is present
    let has_shell_attr = ast
        .attrs
        .iter()
        .any(|attr| attr.path().is_ident("ractor_shell"));

    // we don't support the derive macro on structs or unions
    if let syn::Data::Enum(enum_data) = &ast.data {
        // Build the "serialize()" handler for each variant
        let serialized_variants = enum_data
            .variants
            .iter()
            .map(impl_variant_serialize)
            .collect::<Result<Vec<_>, _>>()?;

        // Build the deserialize handlers for both casts and calls
        let casts = enum_data
            .variants
            .iter()
            .filter_map(|variant| {
                let is_call = variant.attrs.iter().any(|attr| attr.path().is_ident("rpc"));
                if !is_call {
                    Some(impl_cast_variant_deserialize(variant))
                } else {
                    None
                }
            })
            .collect::<Result<Vec<_>, _>>()?;

        let calls = enum_data
            .variants
            .iter()
            .filter_map(|variant| {
                let is_call = variant.attrs.iter().any(|attr| attr.path().is_ident("rpc"));
                if is_call {
                    Some(impl_call_variant_deserialize(variant))
                } else {
                    None
                }
            })
            .collect::<Result<Vec<_>, _>>()?;

        // Generate the Message impl
        let message_impl = quote! {
            impl #impl_generics ractor::Message for #name #ty_generics #where_clause {
                fn serializable() -> bool {
                    // Network serializable message
                    true
                }

                fn serialize(self) -> Result<ractor::message::SerializedMessage, ractor::message::BoxedDowncastErr> {
                    use ::ractor::BytesConvertable;
                    match self {
                        #( #serialized_variants ),*
                    }
                }

                fn deserialize(bytes: ractor::message::SerializedMessage) -> Result<Self, ractor::message::BoxedDowncastErr> {
                    use ::ractor::BytesConvertable;
                    match bytes {
                        ractor::message::SerializedMessage::Cast {variant, args, metadata} => {
                            match variant.as_str() {
                                #(#casts,)*
                                _ => {
                                    // unknown CAST type
                                    Err(ractor::message::BoxedDowncastErr)
                                }
                            }
                        }
                        ractor::message::SerializedMessage::Call {variant, args, reply, metadata} => {
                            match variant.as_str() {
                                #(#calls,)*
                                _ => {
                                    // unknown CALL type
                                    Err(ractor::message::BoxedDowncastErr)
                                }
                            }
                        }
                        _ => {
                            // call-reply isn't supported here
                            Err(ractor::message::BoxedDowncastErr)
                        }
                    }
                }
            }
        };

        // Optionally generate SchemaProvider impl and dispatcher if #[ractor_shell] is present
        let schema_impl = if has_shell_attr {
            let schema_provider_impl =
                impl_schema_provider(name, &impl_generics, &ty_generics, where_clause, enum_data)?;
            let dispatcher_module = impl_dispatcher_module(name, enum_data)?;
            quote! {
                #dispatcher_module
                #schema_provider_impl
            }
        } else {
            quote! {}
        };

        Ok((quote! {
            #message_impl
            #schema_impl
        })
        .into())
    } else {
        Err(syn::Error::new(
            name.span(),
            "RactorClusterMessage can only be derived for enums, not structs or unions",
        ))
    }
}

fn impl_variant_serialize(variant: &Variant) -> syn::Result<impl ToTokens> {
    let name = &variant.ident;
    let variant_name = name.to_string();
    let is_call = variant.attrs.iter().any(|attr| attr.path().is_ident("rpc"));

    if is_call {
        match &variant.fields {
            Fields::Unit => Err(syn::Error::new(
                variant.span(),
                "RPC calls must have at least one field: a `RpcReplyPort<T>` as the last argument.\n\
                 \n\
                 Example:\n  #[rpc]\n  YourRpc(RpcReplyPort<YourReturnType>),",
            )),
            Fields::Unnamed(unnamed_fields) => {
                // we only support un-named fields, where the last field is the "reply" port
                let mut fields = unnamed_fields
                    .unnamed
                    .iter()
                    .enumerate()
                    .map(|(i, arg)| (format_ident!("field{}", i), &arg.ty))
                    .collect::<Vec<_>>();

                // the last field is the port
                let _ = fields.pop();
                let last_field = unnamed_fields.unnamed.last().ok_or_else(|| {
                    syn::Error::new(
                        variant.span(),
                        "RPC calls must have at least one field for the RpcReplyPort",
                    )
                })?;

                let port_type = if let syn::Type::Path(path_data) = &last_field.ty {
                    get_generic_reply_port_type(path_data)?
                } else {
                    return Err(syn::Error::new(
                        last_field.ty.span(),
                        "Expected RpcReplyPort<T> type, but found a non-path type",
                    ));
                };

                let port = format_ident!("reply");
                let target_port = convert_serialize_port(&port, &port_type);

                if fields.is_empty() {
                    // no arguments, just a port
                    Ok(quote! {
                        Self::#name(#port) => {
                            let target_port = #target_port;
                            Ok(ractor::message::SerializedMessage::Call {
                                variant: #variant_name.to_string(),
                                args: vec![],
                                reply: target_port,
                                metadata: None,
                            })
                        }
                    })
                } else {
                    let field_names = fields.iter().map(|(a, _)| a);
                    let packed = fields.iter().map(|(field, arg)| pack_args(field, arg));
                    Ok(quote! {
                        Self::#name(#(#field_names),*, #port) => {
                            let mut data = vec![];
                            #(#packed;)*
                            let target_port = #target_port;
                            Ok(ractor::message::SerializedMessage::Call {
                                variant: #variant_name.to_string(),
                                args: data,
                                reply: target_port,
                                metadata: None,
                            })
                        }
                    })
                }
            }
            Fields::Named(_) => Err(syn::Error::new(
                variant.span(),
                "Named fields are not currently supported for RactorClusterMessage.\n\
                 \n\
                 Please use unnamed (tuple-style) fields instead:\n  \
                 #[rpc]\n  YourVariant(YourType, RpcReplyPort<ReturnType>),",
            )),
        }
    } else {
        match &variant.fields {
            Fields::Unit => {
                // empty, just use the index value
                Ok(quote! {
                    Self::#name => {
                        Ok(ractor::message::SerializedMessage::Cast {
                            variant: #variant_name.to_string(),
                            args: vec![],
                            metadata: None,
                        })
                    }
                })
            }
            Fields::Unnamed(unnamed_fields) => {
                // we only support un-named fields, where the last field is the "reply" port
                let fields = unnamed_fields
                    .unnamed
                    .iter()
                    .enumerate()
                    .map(|(i, arg)| (format_ident!("field{}", i), &arg.ty))
                    .collect::<Vec<_>>();

                let field_names = fields.iter().map(|(a, _)| a);
                let packed = fields.iter().map(|(field, arg)| pack_args(field, arg));
                Ok(quote! {
                    Self::#name(#(#field_names),*) => {
                        let mut data = vec![];

                        #(#packed ;)*

                        Ok(ractor::message::SerializedMessage::Cast {
                            variant: #variant_name.to_string(),
                            args: data,
                            metadata: None,
                        })
                    }
                })
            }
            Fields::Named(_) => Err(syn::Error::new(
                variant.span(),
                "Named fields are not currently supported for RactorClusterMessage.\n\
                 \n\
                 Please use unnamed (tuple-style) fields instead:\n  \
                 YourVariant(YourType1, YourType2),",
            )),
        }
    }
}

fn impl_cast_variant_deserialize(variant: &Variant) -> syn::Result<impl ToTokens> {
    let name = &variant.ident;
    let variant_name = name.to_string();
    match &variant.fields {
        Fields::Unit => {
            // empty, just the index value
            Ok(quote! {
                #variant_name => {
                    Ok(Self::#name)
                }
            })
        }
        Fields::Unnamed(unnamed_fields) => {
            let fields = unnamed_fields
                .unnamed
                .iter()
                .enumerate()
                .map(|(i, arg)| (format_ident!("field{}", i), &arg.ty))
                .collect::<Vec<_>>();
            let field_names = fields.iter().map(|(a, _)| a);
            let unpacked = fields.iter().map(|(field, arg)| unpack_arg(field, arg));
            Ok(quote! {
                #variant_name => {
                    let mut ptr = 0usize;
                    #(#unpacked;)*
                    Ok(Self::#name(#(#field_names),*))
                }
            })
        }
        Fields::Named(_) => Err(syn::Error::new(
            variant.span(),
            "Named fields are not currently supported for RactorClusterMessage.\n\
             \n\
             Please use unnamed (tuple-style) fields instead:\n  \
             YourVariant(YourType1, YourType2),",
        )),
    }
}

fn impl_call_variant_deserialize(variant: &Variant) -> syn::Result<impl ToTokens> {
    let name = &variant.ident;
    let variant_name = name.to_string();
    match &variant.fields {
        Fields::Unit => Err(syn::Error::new(
            variant.span(),
            "RPC calls must have at least one field: a `RpcReplyPort<T>` as the last argument.\n\
             \n\
             Example:\n  #[rpc]\n  YourRpc(RpcReplyPort<YourReturnType>),",
        )),
        Fields::Unnamed(unnamed_fields) => {
            let mut fields = unnamed_fields
                .unnamed
                .iter()
                .enumerate()
                .map(|(i, arg)| (format_ident!("field{}", i), &arg.ty))
                .collect::<Vec<_>>();

            let port = format_ident!("reply");
            let last_field = unnamed_fields.unnamed.last().ok_or_else(|| {
                syn::Error::new(
                    variant.span(),
                    "RPC calls must have at least one field for the RpcReplyPort",
                )
            })?;

            let port_type = if let syn::Type::Path(path_data) = &last_field.ty {
                get_generic_reply_port_type(path_data)?
            } else {
                return Err(syn::Error::new(
                    last_field.ty.span(),
                    "Expected RpcReplyPort<T> type, but found a non-path type",
                ));
            };

            let target_port = convert_deserialize_port(&port, &port_type);

            // the last field is the port, pop it off
            let _ = fields.pop();
            if fields.is_empty() {
                Ok(quote! {
                    #variant_name => {
                        let target_port = #target_port;
                        Ok(Self::#name(target_port))
                    }
                })
            } else {
                let field_names = fields.iter().map(|(a, _)| a);
                let unpacked = fields.iter().map(|(field, arg)| unpack_arg(field, arg));
                Ok(quote! {
                    #variant_name => {
                        let mut ptr = 0usize;
                        #(#unpacked;)*
                        let target_port = #target_port;
                        Ok(Self::#name(#(#field_names),*, target_port))
                    }
                })
            }
        }
        Fields::Named(_) => Err(syn::Error::new(
            variant.span(),
            "Named fields are not currently supported for RactorClusterMessage.\n\
             \n\
             Please use unnamed (tuple-style) fields instead:\n  \
             #[rpc]\n  YourVariant(YourType, RpcReplyPort<ReturnType>),",
        )),
    }
}

fn pack_args(field: &Ident, target_type: &syn::Type) -> impl ToTokens {
    quote! {
        {
            let arg_data = <#target_type as ractor::BytesConvertable>::into_bytes(#field);
            let arg_len = (arg_data.len() as u64).to_be_bytes();
            data.extend(arg_len);
            data.extend(arg_data);
        }
    }
}

fn unpack_arg(field: &Ident, target_type: &syn::Type) -> impl ToTokens {
    quote! {
        let #field = {
            let mut len_bytes = [0u8; 8];
            len_bytes.copy_from_slice(&args[ptr..ptr+8]);
            let len = u64::from_be_bytes(len_bytes) as usize;

            ptr += 8;
            let data_bytes = args[ptr..ptr+len].to_vec();
            let t_result = <#target_type as ractor::BytesConvertable>::from_bytes(data_bytes);
            ptr += len;
            t_result
        };
    }
}

fn convert_deserialize_port(
    the_port: &Ident,
    port_type: &AngleBracketedGenericArguments,
) -> impl ToTokens {
    let generic_args = &port_type.args;
    // TODO: catch unwind for the conversion? returning Err(BoxedDowncastErr)
    quote! {
        {
            let (tx, rx) = ractor::concurrency::oneshot::#port_type();
            let o_timeout = #the_port.get_timeout();
            ractor::concurrency::spawn(async move {
                if let Some(timeout) = o_timeout {
                    if let Ok(Ok(result)) = ractor::concurrency::timeout(timeout, rx).await {
                        let _ = #the_port.send(<#generic_args as BytesConvertable>::into_bytes(result));
                    }
                } else {
                    if let Ok(result) = rx.await {
                        let _ = #the_port.send(<#generic_args as BytesConvertable>::into_bytes(result));
                    }
                }
            });
            if let Some(timeout) = o_timeout {
                ractor::RpcReplyPort::<_>::from((tx, timeout))
            } else {
                ractor::RpcReplyPort::<_>::from(tx)
            }
        }
    }
}

fn convert_serialize_port(
    the_port: &Ident,
    target_type: &AngleBracketedGenericArguments,
) -> impl ToTokens {
    // TODO: catch unwind for the conversion? returning Err(BoxedDowncastErr)
    let generic_args = &target_type.args;
    quote! {
        {
            let (tx, rx) = ractor::concurrency::oneshot();
            let o_timeout = #the_port.get_timeout();
            ractor::concurrency::spawn(async move {
                if let Some(timeout) = o_timeout {
                    if let Ok(Ok(result)) = ractor::concurrency::timeout(timeout, rx).await {
                        let typed_result = <#generic_args as ractor::BytesConvertable>::from_bytes(result);
                        let _ = #the_port.send(typed_result);
                    }
                } else {
                    if let Ok(result) = rx.await {
                        let typed_result = <#generic_args as ractor::BytesConvertable>::from_bytes(result);
                        let _ = #the_port.send(typed_result);
                    }
                }
            });
            if let Some(timeout) = o_timeout {
                ractor::RpcReplyPort::<_>::from((tx, timeout))
            } else {
                ractor::RpcReplyPort::<_>::from(tx)
            }
        }
    }
}

fn get_generic_reply_port_type(
    path_data: &TypePath,
) -> syn::Result<AngleBracketedGenericArguments> {
    let last_segment = path_data.path.segments.last().ok_or_else(|| {
        syn::Error::new(
            path_data.span(),
            "Expected a type path with at least one segment (e.g., RpcReplyPort<T>)",
        )
    })?;

    if let syn::PathArguments::AngleBracketed(generic_args) = &last_segment.arguments {
        Ok(generic_args.clone())
    } else {
        Err(syn::Error::new(
            last_segment.span(),
            "RpcReplyPort must have generic type arguments.\n\
             \n\
             Expected: RpcReplyPort<YourReturnType>\n\
             \n\
             The generic argument specifies what type will be returned by the RPC call.",
        ))
    }
}

// ======================== SchemaProvider Generation ======================== //

fn impl_schema_provider(
    name: &Ident,
    impl_generics: &syn::ImplGenerics,
    ty_generics: &syn::TypeGenerics,
    where_clause: Option<&syn::WhereClause>,
    enum_data: &syn::DataEnum,
) -> syn::Result<impl ToTokens> {
    // Build the schema JSON string
    let schema_json = build_schema_json(enum_data)?;

    // Build from_json match arms for cast (non-RPC) variants
    let from_json_arms = enum_data
        .variants
        .iter()
        .map(|variant| impl_variant_from_json(name, variant))
        .collect::<Result<Vec<_>, _>>()?;

    // Build to_json match arms for all variants
    let to_json_arms = enum_data
        .variants
        .iter()
        .map(|variant| impl_variant_to_json(name, variant))
        .collect::<Result<Vec<_>, _>>()?;

    // Check if there are any RPC variants to determine if we should generate a dispatcher
    let has_rpc_variants = enum_data
        .variants
        .iter()
        .any(|v| v.attrs.iter().any(|attr| attr.path().is_ident("rpc")));

    // Generate the dispatcher() method if there are RPC variants
    let dispatcher_method = if has_rpc_variants {
        let module_name = format_ident!(
            "__ractor_shell_{}_dispatcher",
            name.to_string().to_lowercase()
        );
        quote! {
            fn dispatcher() -> Option<ractor::RpcDispatcher> {
                Some(std::sync::Arc::new(|cell, variant, args| {
                    Box::pin(#module_name::dispatch_rpc(cell, variant, args))
                }))
            }
        }
    } else {
        quote! {}
    };

    // Note: We don't wrap this in #[cfg(feature = "shell-introspection")] because
    // that feature is defined in the `ractor` crate, not in the crate using this derive.
    // If the feature isn't enabled, ractor::SchemaProvider won't exist and compilation
    // will fail with a clear error message.
    Ok(quote! {
        impl #impl_generics ractor::SchemaProvider for #name #ty_generics #where_clause {
            fn message_schema() -> &'static str {
                #schema_json
            }

            fn from_json(variant: &str, args: serde_json::Value) -> Result<Self, ractor::JsonDeserializeError> {
                match variant {
                    #( #from_json_arms ),*
                    _ => Err(ractor::JsonDeserializeError::unknown_variant(variant))
                }
            }

            fn to_json(&self) -> serde_json::Value {
                match self {
                    #( #to_json_arms ),*
                }
            }

            #dispatcher_method
        }
    })
}

fn build_schema_json(enum_data: &syn::DataEnum) -> syn::Result<String> {
    let mut variants_json = Vec::new();

    for variant in &enum_data.variants {
        let variant_name = variant.ident.to_string();
        let is_rpc = variant.attrs.iter().any(|attr| attr.path().is_ident("rpc"));
        let field_names = parse_fields_attribute(variant)?;

        let fields_json = match &variant.fields {
            Fields::Unit => "{}".to_string(),
            Fields::Unnamed(unnamed) => {
                let mut field_entries = Vec::new();
                let field_count = if is_rpc {
                    // Exclude the last field (RpcReplyPort) for RPC variants
                    unnamed.unnamed.len().saturating_sub(1)
                } else {
                    unnamed.unnamed.len()
                };

                // Validate field_names count if provided
                if let Some(ref names) = field_names {
                    if names.len() != field_count {
                        return Err(syn::Error::new(
                            variant.ident.span(),
                            format!(
                                "#[fields(...)] has {} names but variant '{}' has {} fields{}",
                                names.len(),
                                variant_name,
                                field_count,
                                if is_rpc {
                                    " (excluding RpcReplyPort)"
                                } else {
                                    ""
                                }
                            ),
                        ));
                    }
                }

                for (i, field) in unnamed.unnamed.iter().take(field_count).enumerate() {
                    let type_str = type_to_string(&field.ty);
                    let field_key = get_field_key(i, &field_names);
                    field_entries.push(format!("\"{}\":\"{}\"", field_key, type_str));
                }
                format!("{{{}}}", field_entries.join(","))
            }
            Fields::Named(named) => {
                let mut field_entries = Vec::new();
                for field in &named.named {
                    if let Some(ident) = &field.ident {
                        let type_str = type_to_string(&field.ty);
                        field_entries.push(format!("\"{}\":\"{}\"", ident, type_str));
                    }
                }
                format!("{{{}}}", field_entries.join(","))
            }
        };

        // Get reply type for RPC variants
        let reply_type_json = if is_rpc {
            if let Fields::Unnamed(unnamed) = &variant.fields {
                if let Some(last_field) = unnamed.unnamed.last() {
                    if let syn::Type::Path(path) = &last_field.ty {
                        if let Ok(generic_args) = get_generic_reply_port_type(path) {
                            let reply_type_str = generic_args.args.to_token_stream().to_string();
                            format!(",\"reply_type\":\"{}\"", reply_type_str.replace(' ', ""))
                        } else {
                            String::new()
                        }
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                }
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        variants_json.push(format!(
            "\"{}\":{{\"fields\":{},\"rpc\":{}{}}}",
            variant_name, fields_json, is_rpc, reply_type_json
        ));
    }

    Ok(format!("{{\"variants\":{{{}}}}}", variants_json.join(",")))
}

fn type_to_string(ty: &syn::Type) -> String {
    // Convert a type to a simple string representation
    ty.to_token_stream()
        .to_string()
        .replace(' ', "")
        .replace("ractor::", "")
        .replace("crate::", "")
}

fn impl_variant_from_json(_enum_name: &Ident, variant: &Variant) -> syn::Result<impl ToTokens> {
    let variant_name = &variant.ident;
    let variant_name_str = variant_name.to_string();
    let is_rpc = variant.attrs.iter().any(|attr| attr.path().is_ident("rpc"));
    let custom_field_names = parse_fields_attribute(variant)?;

    // RPC variants cannot be deserialized from JSON directly (need RpcReplyPort)
    if is_rpc {
        return Ok(quote! {
            #variant_name_str => {
                Err(ractor::JsonDeserializeError::new(
                    format!("'{}' is an RPC variant and cannot be deserialized from JSON directly. Use the schema-aware RPC call mechanism.", #variant_name_str)
                ))
            }
        });
    }

    match &variant.fields {
        Fields::Unit => Ok(quote! {
            #variant_name_str => {
                Ok(Self::#variant_name)
            }
        }),
        Fields::Unnamed(unnamed) => {
            let field_extractions: Vec<_> = unnamed
                .unnamed
                .iter()
                .enumerate()
                .map(|(i, field)| {
                    let field_name = format_ident!("field{}", i);
                    let field_key = get_field_key(i, &custom_field_names);
                    let field_type = &field.ty;
                    generate_json_field_extraction(&field_name, &field_key, field_type)
                })
                .collect();

            let field_names: Vec<_> = (0..unnamed.unnamed.len())
                .map(|i| format_ident!("field{}", i))
                .collect();

            Ok(quote! {
                #variant_name_str => {
                    #( #field_extractions )*
                    Ok(Self::#variant_name(#( #field_names ),*))
                }
            })
        }
        Fields::Named(named) => {
            let field_extractions: Vec<_> = named
                .named
                .iter()
                .filter_map(|field| {
                    field.ident.as_ref().map(|ident| {
                        let field_name_str = ident.to_string();
                        let field_type = &field.ty;
                        generate_json_field_extraction(ident, &field_name_str, field_type)
                    })
                })
                .collect();

            let field_names: Vec<_> = named
                .named
                .iter()
                .filter_map(|f| f.ident.as_ref())
                .collect();

            Ok(quote! {
                #variant_name_str => {
                    #( #field_extractions )*
                    Ok(Self::#variant_name { #( #field_names ),* })
                }
            })
        }
    }
}

fn generate_json_field_extraction(
    field_name: &Ident,
    field_key: &str,
    field_type: &syn::Type,
) -> impl ToTokens {
    // Generate code to extract a field from JSON
    // We use serde_json's from_value for type conversion
    quote! {
        let #field_name: #field_type = {
            let value = args.get(#field_key)
                .ok_or_else(|| ractor::JsonDeserializeError::missing_field(#field_key))?;
            serde_json::from_value(value.clone())
                .map_err(|e| ractor::JsonDeserializeError::new(
                    format!("field '{}': {}", #field_key, e)
                ))?
        };
    }
}

fn impl_variant_to_json(_enum_name: &Ident, variant: &Variant) -> syn::Result<impl ToTokens> {
    let variant_name = &variant.ident;
    let variant_name_str = variant_name.to_string();
    let is_rpc = variant.attrs.iter().any(|attr| attr.path().is_ident("rpc"));
    let custom_field_names = parse_fields_attribute(variant)?;

    match &variant.fields {
        Fields::Unit => Ok(quote! {
            Self::#variant_name => {
                serde_json::json!({
                    "variant": #variant_name_str
                })
            }
        }),
        Fields::Unnamed(unnamed) => {
            // For RPC variants, exclude the last field (RpcReplyPort)
            let field_count = if is_rpc {
                unnamed.unnamed.len().saturating_sub(1)
            } else {
                unnamed.unnamed.len()
            };

            let field_names: Vec<_> = (0..field_count)
                .map(|i| format_ident!("field{}", i))
                .collect();

            let field_patterns: Vec<_> = if is_rpc {
                // Include all fields in pattern but only use non-port ones
                let all_names: Vec<_> = (0..unnamed.unnamed.len())
                    .map(|i| {
                        if i < field_count {
                            format_ident!("field{}", i)
                        } else {
                            format_ident!("_reply")
                        }
                    })
                    .collect();
                all_names
            } else {
                field_names.clone()
            };

            let json_fields: Vec<_> = (0..field_count)
                .map(|i| {
                    let field_name = format_ident!("field{}", i);
                    let field_key = get_field_key(i, &custom_field_names);
                    quote! { #field_key: #field_name }
                })
                .collect();

            Ok(quote! {
                Self::#variant_name(#( #field_patterns ),*) => {
                    serde_json::json!({
                        "variant": #variant_name_str,
                        #( #json_fields ),*
                    })
                }
            })
        }
        Fields::Named(named) => {
            let field_names: Vec<_> = named
                .named
                .iter()
                .filter_map(|f| f.ident.as_ref())
                .collect();

            let json_fields: Vec<_> = field_names
                .iter()
                .map(|name| {
                    let name_str = name.to_string();
                    quote! { #name_str: #name }
                })
                .collect();

            Ok(quote! {
                Self::#variant_name { #( #field_names ),* } => {
                    serde_json::json!({
                        "variant": #variant_name_str,
                        #( #json_fields ),*
                    })
                }
            })
        }
    }
}

// ======================== Dispatcher Generation ======================== //

/// Generate a hidden dispatcher module for typed RPC calls via shell.
///
/// This generates a module containing a `dispatch_rpc` function that:
/// 1. Converts ActorCell to typed ActorRef
/// 2. Matches on RPC variant name
/// 3. Makes typed RPC call using ractor::call!
/// 4. Serializes the result to JSON
fn impl_dispatcher_module(
    message_type: &Ident,
    enum_data: &syn::DataEnum,
) -> syn::Result<impl ToTokens> {
    // Collect only RPC variants
    let rpc_variants: Vec<_> = enum_data
        .variants
        .iter()
        .filter(|v| v.attrs.iter().any(|attr| attr.path().is_ident("rpc")))
        .collect();

    // If no RPC variants, don't generate dispatcher
    if rpc_variants.is_empty() {
        return Ok(quote! {});
    }

    // Generate match arms for each RPC variant
    let dispatcher_arms: Vec<_> = rpc_variants
        .iter()
        .map(|variant| impl_dispatcher_arm(message_type, variant))
        .collect::<Result<Vec<_>, _>>()?;

    // Generate module name based on message type (lowercase with underscores)
    let module_name = format_ident!(
        "__ractor_shell_{}_dispatcher",
        message_type.to_string().to_lowercase()
    );

    Ok(quote! {
        #[doc(hidden)]
        mod #module_name {
            use super::*;

            pub async fn dispatch_rpc(
                cell: ractor::ActorCell,
                variant: String,
                args: serde_json::Value,
            ) -> Result<serde_json::Value, String> {
                let actor_ref: ractor::ActorRef<#message_type> = ractor::ActorRef::from(cell);
                let timeout = Some(std::time::Duration::from_secs(5));

                match variant.as_str() {
                    #( #dispatcher_arms ),*
                    _ => Err(format!("Unknown RPC variant: {}", variant))
                }
            }
        }
    })
}

/// Generate a match arm for a single RPC variant in the dispatcher.
fn impl_dispatcher_arm(message_type: &Ident, variant: &Variant) -> syn::Result<impl ToTokens> {
    let variant_name = &variant.ident;
    let variant_name_str = variant_name.to_string();
    let custom_field_names = parse_fields_attribute(variant)?;

    // Get the fields for this variant
    let fields = match &variant.fields {
        Fields::Unnamed(unnamed) => {
            if unnamed.unnamed.is_empty() {
                return Err(syn::Error::new(
                    variant.span(),
                    "RPC variant must have at least one field (RpcReplyPort<T>)",
                ));
            }
            &unnamed.unnamed
        }
        _ => {
            return Err(syn::Error::new(
                variant.span(),
                "RPC variants must use unnamed fields",
            ));
        }
    };

    // Number of argument fields (excluding the last field which is RpcReplyPort)
    let arg_count = fields.len() - 1;

    if arg_count == 0 {
        // Single-field RPC: just the reply port
        Ok(quote! {
            #variant_name_str => {
                match actor_ref.call(
                    |reply_port| #message_type::#variant_name(reply_port),
                    timeout
                ).await {
                    Ok(ractor::rpc::CallResult::Success(result)) => {
                        serde_json::to_value(&result)
                            .map_err(|e| format!("Serialization failed: {}", e))
                    }
                    Ok(ractor::rpc::CallResult::Timeout) => {
                        Err("RPC timeout".to_string())
                    }
                    Ok(ractor::rpc::CallResult::SenderError) => {
                        Err("Actor stopped or unreachable".to_string())
                    }
                    Err(e) => Err(format!("RPC failed: {:?}", e))
                }
            }
        })
    } else {
        // RPC with arguments: extract fields from JSON, then call
        let field_extractions: Vec<_> = fields
            .iter()
            .take(arg_count)
            .enumerate()
            .map(|(i, field)| {
                let field_name = format_ident!("arg{}", i);
                let field_key = get_field_key(i, &custom_field_names);
                let field_type = &field.ty;
                generate_dispatcher_field_extraction(&field_name, &field_key, field_type)
            })
            .collect();

        let field_names: Vec<_> = (0..arg_count).map(|i| format_ident!("arg{}", i)).collect();

        Ok(quote! {
            #variant_name_str => {
                #( #field_extractions )*
                match actor_ref.call(
                    |reply_port| #message_type::#variant_name(#( #field_names, )* reply_port),
                    timeout
                ).await {
                    Ok(ractor::rpc::CallResult::Success(result)) => {
                        serde_json::to_value(&result)
                            .map_err(|e| format!("Serialization failed: {}", e))
                    }
                    Ok(ractor::rpc::CallResult::Timeout) => {
                        Err("RPC timeout".to_string())
                    }
                    Ok(ractor::rpc::CallResult::SenderError) => {
                        Err("Actor stopped or unreachable".to_string())
                    }
                    Err(e) => Err(format!("RPC failed: {:?}", e))
                }
            }
        })
    }
}

/// Generate field extraction code for dispatcher (returns String errors).
fn generate_dispatcher_field_extraction(
    field_name: &Ident,
    field_key: &str,
    field_type: &syn::Type,
) -> impl ToTokens {
    quote! {
        let #field_name: #field_type = {
            let value = args.get(#field_key)
                .ok_or_else(|| format!("Missing required field '{}'", #field_key))?;
            serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid field '{}': {}", #field_key, e))?
        };
    }
}
