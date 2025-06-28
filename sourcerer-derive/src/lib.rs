//! A derive macro for the `Event` trait in the `sourcerer` crate.
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

/// Derives the `Event` trait for an enum.
///
/// This macro automatically implements the `event_type` method, which returns
/// a string slice representing the variant's name.
#[proc_macro_derive(Event)]
pub fn event_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let variants = match &input.data {
        Data::Enum(data) => &data.variants,
        _ => panic!("Event derive macro can only be used on enums"),
    };

    let event_type_arms = variants.iter().map(|variant| {
        let variant_name = &variant.ident;
        let fields = match &variant.fields {
            Fields::Named(_) => quote! { { .. } },
            Fields::Unnamed(_) => quote! { (..) },
            Fields::Unit => quote! {},
        };
        quote! {
            #name::#variant_name #fields => stringify!(#variant_name)
        }
    });

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let expanded = quote! {
        impl #impl_generics sourcerer::Event for #name #ty_generics #where_clause {
            fn event_type(&self) -> &'static str {
                match self {
                    #(#event_type_arms),*
                }
            }
        }
    };

    TokenStream::from(expanded)
}
