use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

#[proc_macro_derive(FuzzEq, attributes(fuzz_eq))]
pub fn derive_fuzz_eq(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = input.ident;
    let generics = add_fuzz_eq_bounds(input.generics);

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

fn add_fuzz_eq_bounds(mut generics: syn::Generics) -> syn::Generics {
    let params = generics.params.clone();
    let where_clause = generics.make_where_clause();

    for param in &params {
        if let syn::GenericParam::Type(type_param) = param {
            let ident = &type_param.ident;

            where_clause.predicates.push(syn::parse_quote! {
                #ident: FuzzEq
            });
        }
    }

    generics
}

fn derive_struct(fields: &Fields) -> proc_macro2::TokenStream {
    match fields {
        Fields::Named(fields) => {
            let comparisons = fields.named.iter()
                .filter(|f| !field_has_attr(f, "ignore"))
                .map(|f| {
                    let name = &f.ident;
                    if field_has_attr(f, "use_eq") {
                        quote! { &self.#name == &other.#name }
                    } else {
                        quote! { self.#name.fuzz_eq(&other.#name) }
                    }
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

fn field_has_attr(field: &syn::Field, name: &str) -> bool {
    field.attrs.iter().any(|attr| {
        attr.path().is_ident("fuzz_eq")
            && attr.parse_args::<syn::Ident>().map_or(false, |ident| ident == name)
    })
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
                    if variant_has_attr(&variant, "use_eq") {
                        quote! { #a == #b }
                    } else {
                        quote! { #a.fuzz_eq(&#b) }
                    }
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
                let lhs: Vec<_> = fields.named.iter()
                    .map(|f| syn::Ident::new(
                        &format!("a_{}", f.ident.as_ref().unwrap()),
                        vname.span(),
                    ))
                    .collect();
                let rhs: Vec<_> = fields.named.iter()
                    .map(|f| syn::Ident::new(
                        &format!("b_{}", f.ident.as_ref().unwrap()),
                        vname.span(),
                    ))
                    .collect();

                let names: Vec<_> =
                    fields.named.iter().map(|f| f.ident.as_ref().unwrap()).collect();

                let comparisons = lhs.iter().zip(rhs.iter()).map(|(a, b)| {
                    if variant_has_attr(&variant, "use_eq") {
                        quote! { #a == #b }
                    } else {
                        quote! { #a.fuzz_eq(&#b) }
                    }
                });

                quote! {
                    (
                        #enum_name::#vname { #(#names: #lhs),* },
                        #enum_name::#vname { #(#names: #rhs),* }
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

fn variant_has_attr(variant: &syn::Variant, name: &str) -> bool {
    variant.attrs.iter().any(|attr| {
        attr.path().is_ident("fuzz_eq")
            && attr.parse_args::<syn::Ident>().map_or(false, |ident| ident == name)
    })
}
