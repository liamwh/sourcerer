//! Procedural macros for the `sourcerer` event–sourcing framework.
//!
//! # `#[derive(Event)]`
//! The `Event` derive automatically implements the `sourcerer::Event` trait for an enum.
//! It generates:
//!
//! * `event_type` – returns the **variant name** as a `&'static str`.
//! * `event_version` – configurable per–enum or per–variant (defaults to `1`).
//! * `event_source` – configurable per–enum or per–variant (defaults to
//!   `"urn:sourcerer:event"`).
//!
//! ## Attribute syntax
//!
//! ```ignore
//! #[derive(Event)]
//! // Enum-level defaults
//! #[event(version = 2, source = "urn:my-service")]
//! enum AccountEvent {
//!     // Inherits version = 2, source = "urn:my-service".
//!     Opened,
//!
//!     // Override only the version; source inherits from the enum.
//!     #[event(version = 3)]
//!     Credited { amount: u64 },
//!
//!     // Override both.
//!     #[event(version = 4, source = "urn:custom")]
//!     Debited(u64),
//! }
//! ```
//!
//! ### Generated behaviour
//!
//! ```ignore
//! assert_eq!(AccountEvent::Opened.event_type(), "Opened");
//! assert_eq!(AccountEvent::Opened.event_version(), 2);
//! assert_eq!(AccountEvent::Opened.event_source(), "urn:my-service");
//!
//! assert_eq!(AccountEvent::Credited { amount: 10 }.event_version(), 3);
//! assert_eq!(AccountEvent::Credited { amount: 10 }.event_source(), "urn:my-service");
//!
//! assert_eq!(AccountEvent::Debited(5).event_version(), 4);
//! assert_eq!(AccountEvent::Debited(5).event_source(), "urn:custom");
//! ```
//!
//! ## Notes
//! * The macro works for unit, tuple and struct variants.
//! * Unknown keys in the `event(...)` attribute are ignored, which future-proofs
//!   the API for additional options.
//! * All variants **must** implement `Serialize` and `DeserializeOwned` (the
//!   blanket `#[derive(Serialize, Deserialize)]` on the enum is usually
//!   sufficient).
//!
//! ---
//! Currently `sourcerer-derive` only provides the `Event` macro. More helpers
//! may be added in the future.
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{
    Data, DeriveInput, Fields, Lit, MetaNameValue, Token, parse_macro_input, punctuated::Punctuated,
};

/// Derives the `Event` trait for an enum.
///
/// This macro automatically implements the `event_type` method, which returns
/// a string slice representing the variant's name.
#[proc_macro_derive(Event, attributes(event))]
pub fn event_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    // Helper to parse meta list for keys version, source
    fn extract_meta(
        list: &Punctuated<MetaNameValue, Token![,]>,
        version: &mut Option<u16>,
        source: &mut Option<String>,
    ) {
        for nv in list {
            let ident = nv.path.get_ident().map(|i| i.to_string());
            if let Some(key) = ident {
                match key.as_str() {
                    "version" => {
                        if let syn::Expr::Lit(expr_lit) = &nv.value {
                            if let Lit::Int(li) = &expr_lit.lit {
                                *version = Some(li.base10_parse::<u16>().expect("invalid int"));
                            }
                        }
                    }
                    "source" => {
                        if let syn::Expr::Lit(expr_lit) = &nv.value {
                            if let Lit::Str(ls) = &expr_lit.lit {
                                *source = Some(ls.value());
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // Enum-level defaults
    let mut enum_version: Option<u16> = None;
    let mut enum_source: Option<String> = None;

    for attr in &input.attrs {
        if attr.path().is_ident("event") {
            let parser = Punctuated::<MetaNameValue, Token![,]>::parse_terminated;
            let list = attr
                .parse_args_with(parser)
                .expect("invalid event attribute");
            extract_meta(&list, &mut enum_version, &mut enum_source);
        }
    }

    let default_version = enum_version.unwrap_or(1);
    let default_source = enum_source.unwrap_or_else(|| "urn:sourcerer:event".to_string());

    let variants = match &input.data {
        Data::Enum(data) => &data.variants,
        _ => panic!("Event derive macro can only be used on enums"),
    };

    // Build arms for event_type, version, source
    let mut type_arms = Vec::new();
    let mut version_arms = Vec::new();
    let mut source_arms = Vec::new();

    for variant in variants {
        let ident = &variant.ident;
        let fields_tokens = match &variant.fields {
            Fields::Named(_) => quote! { { .. } },
            Fields::Unnamed(_) => quote! { (..) },
            Fields::Unit => quote! {},
        };

        // Variant attribute overrides
        let mut var_version = None;
        let mut var_source = None;
        for attr in &variant.attrs {
            if attr.path().is_ident("event") {
                let parser = Punctuated::<MetaNameValue, Token![,]>::parse_terminated;
                let list = attr
                    .parse_args_with(parser)
                    .expect("invalid event attribute");
                extract_meta(&list, &mut var_version, &mut var_source);
            }
        }

        let ver_val = var_version.unwrap_or(default_version);
        let src_val = var_source.unwrap_or_else(|| default_source.clone());
        let src_lit = syn::LitStr::new(&src_val, Span::call_site());

        type_arms.push(quote! { #name::#ident #fields_tokens => stringify!(#ident) });
        version_arms.push(quote! { #name::#ident #fields_tokens => #ver_val });
        source_arms.push(quote! { #name::#ident #fields_tokens => #src_lit });
    }

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let expanded = quote! {
        impl #impl_generics sourcerer::Event for #name #ty_generics #where_clause {
            fn event_type(&self) -> &'static str {
                match self {
                    #(#type_arms),*
                }
            }

            fn event_version(&self) -> u16 {
                match self {
                    #(#version_arms),*
                }
            }

            fn event_source(&self) -> &'static str {
                match self {
                    #(#source_arms),*
                }
            }
        }
    };

    TokenStream::from(expanded)
}
