use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

#[proc_macro_derive(FuzzEq)]
pub fn derive_fuzz_eq(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = input.ident;
    let generics = input.generics;

    let (impl_generics, ty_generics, where_clauses) =
        generics.split_for_impl();

    let body = match input.data {
        Data::Struct(data) => derive_struct(&data.fields),
        Data::Enum(data) => derive_enum(&name, &data.variants),
        Data::Union(_) => {
            return syn::Error::new_spanned(
                name,
                "FuzzEq cannot be derived for unions",
            )
            .to_compile_error()
            .into();
        }
    };

    let expanded = quote! {
        impl #impl_generics FuzzEq for #name #ty_generics #where_clauses {
            fn fuzz_eq(&self, other: &Self) -> bool {
                #body
            }
        }
    };

    expanded.into() // TokenStream::from(expanded)
}

fn derive_struct(fields: &Fields) -> proc_macro2::TokenStream {
    match fields {
        Fields::Named(fields) => {
            let comparisons = fields.named.iter().map(|f| {
                let name = &f.ident;
                quote! { self.#name.fuzz_eq(&other.#name) }
            });

            quote! {
                true #(&& #comparisons)*
            }
        }
        Fields::Unnamed(fields) => {
            let indices = (0..fields.unnamed.len())
                .map(syn::Index::from);

            let comparisons = indices.map(|i| {
                quote! { self.#i.fuzz_eq(&other.#i) }
            });

            quote! {
                true #(&& #comparisons)*
            }
        }
        Fields::Unit => quote!(true),
    }
}

use syn::{Variant, punctuated::Punctuated, token::Comma};

fn derive_enum(
    enum_name: &syn::Ident,
    variants: &Punctuated<Variant, Comma>,
) -> proc_macro2::TokenStream {
    let arms = variants.iter().map(|variant| {
        let vname = &variant.ident;

        match &variant.fields {
            Fields::Unit => {
                quote! {
                    (#enum_name::#vname, #enum_name::#vname) => true
                }
            }
            Fields::Unnamed(fields) => {
                let lhs: Vec<_> = (0..fields.unnamed.len())
                    .map(|i| syn::Ident::new(&format!("a{i}"), vname.span()))
                    .collect();
                let rhs: Vec<_> = (0..fields.unnamed.len())
                    .map(|i| syn::Ident::new(&format!("b{i}"), vname.span()))
                    .collect();

                let comparisons = lhs.iter().zip(rhs.iter()).map(|(a, b)| {
                    quote! { #a.fuzz_eq(&#b) }
                });

                quote! {
                    (
                        #enum_name::#vname( #(#lhs),* ),
                        #enum_name::#vname( #(#rhs),* )
                    ) => {
                        true #(&& #comparisons)*
                    }
                }
            }
            Fields::Named(fields) => {
                let names: Vec<_> =
                    fields.named.iter().map(|f| f.ident.as_ref().unwrap()).collect();

                let comparisons = names.iter().map(|n| {
                    quote! { a.#n.fuzz_eq(&b.#n) }
                });

                quote! {
                    (
                        #enum_name::#vname { #(#names: a.#names),* },
                        #enum_name::#vname { #(#names: b.#names),* }
                    ) => {
                        true #(&& #comparisons)*
                    }
                }
            }
        }
    });

    quote! {
        match (self, other) {
            #(#arms),*,
            _ => false
        }
    }
}
