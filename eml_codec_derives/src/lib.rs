use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};
use syn::{Attribute, Variant, punctuated::Punctuated, token::Comma};

// derive(ToStringFromPrint) ---------------------------------------------------

#[proc_macro_derive(ToStringFromPrint)]
pub fn derive_to_string_from_print(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;
    let (impl_generics, ty_generics, where_clauses) =
        input.generics.split_for_impl();

    let expanded = quote! {
        impl #impl_generics ToString for #name #ty_generics #where_clauses {
            fn to_string(&self) -> String {
                String::from_utf8_lossy(
                    &crate::print::print_to_vec(
                        crate::print::FMT_NOFOLD,
                        self,
                    )
                ).to_string()
            }
        }
    };

    expanded.into()
}

// derive(FuzzEq) --------------------------------------------------------------

#[proc_macro_derive(FuzzEq, attributes(fuzz_eq))]
pub fn derive_fuzz_eq(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = input.ident;
    let generics = add_bounds(input.generics, quote!{ FuzzEq });

    let (impl_generics, ty_generics, where_clauses) =
        generics.split_for_impl();

    let body = match input.data {
        Data::Struct(data) => derive_fuzz_eq_struct(&data.fields),
        Data::Enum(data) => derive_fuzz_eq_enum(&name, &data.variants),
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

fn derive_fuzz_eq_struct(fields: &Fields) -> proc_macro2::TokenStream {
    match fields {
        Fields::Named(fields) => {
            let comparisons = fields.named.iter()
                .filter(|f| !has_attr(&f.attrs, "fuzz_eq", "ignore"))
                .map(|f| {
                    let name = &f.ident;
                    if has_attr(&f.attrs, "fuzz_eq", "use_eq") {
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

fn derive_fuzz_eq_enum(
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
                    if has_attr(&variant.attrs, "fuzz_eq", "use_eq") {
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
                    if has_attr(&variant.attrs, "fuzz_eq", "use_eq") {
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

// derive(ContainsUtf8) --------------------------------------------------------

#[proc_macro_derive(ContainsUtf8, attributes(contains_utf8))]
pub fn derive_contains_utf8(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = input.ident;
    let generics = add_bounds(input.generics, quote!{ ContainsUtf8 });

    let (impl_generics, ty_generics, where_clauses) =
        generics.split_for_impl();

    let body =
        if let Some(b) = has_bool_attr(&input.attrs, "contains_utf8") {
            quote!{ #b }
        } else {
            match input.data {
                Data::Struct(data) => derive_contains_utf8_struct(&data.fields),
                Data::Enum(data) => derive_contains_utf8_enum(&name, &data.variants),
                Data::Union(_) => {
                    return syn::Error::new_spanned(
                        name,
                        "ContainsUtf8 cannot be derived for unions",
                    )
                        .to_compile_error()
                        .into();
                }
            }
        };

    let expanded = quote! {
        impl #impl_generics ContainsUtf8 for #name #ty_generics #where_clauses {
            fn contains_utf8(&self) -> bool {
                #body
            }
        }
    };

    expanded.into()
}

fn derive_contains_utf8_struct(fields: &Fields) -> proc_macro2::TokenStream {
    match fields {
        Fields::Named(fields) => {
            let tests = fields.named.iter()
                .filter(|f| !has_attr(&f.attrs, "contains_utf8", "ignore"))
                .map(|f| {
                    let name = &f.ident;
                    quote! { self.#name.contains_utf8() }
                });

            quote! { false #(|| #tests)* }
        }
        Fields::Unnamed(fields) => {
            let indices = (0..fields.unnamed.len())
                .map(syn::Index::from);

            let comparisons = indices.map(|i| {
                quote! { self.#i.contains_utf8() }
            });

            quote! { false #(|| #comparisons)* }
        }
        Fields::Unit => quote!(false),
    }
}

fn derive_contains_utf8_enum(
    enum_name: &syn::Ident,
    variants: &Punctuated<Variant, Comma>,
) -> proc_macro2::TokenStream {
    let arms = variants.iter().map(|variant| {
        let vname = &variant.ident;

        match &variant.fields {
            Fields::Unit => {
                quote! {
                    #enum_name::#vname => false
                }
            }
            Fields::Unnamed(fields) => {
                let ids: Vec<_> = (0..fields.unnamed.len())
                    .map(|i| syn::Ident::new(&format!("a{i}"), vname.span()))
                    .collect();

                let tests = ids.iter().map(|a| quote! { #a.contains_utf8() });

                quote! {
                    #enum_name::#vname( #(#ids),* ) => false #(|| #tests)*
                }
            }
            Fields::Named(fields) => {
                let ids: Vec<_> = fields.named.iter()
                    .map(|f| syn::Ident::new(
                        &format!("a_{}", f.ident.as_ref().unwrap()),
                        vname.span(),
                    ))
                    .collect();

                let names: Vec<_> =
                    fields.named.iter().map(|f| f.ident.as_ref().unwrap()).collect();

                let tests = ids.iter().map(|a| quote! { #a.contains_utf8() });

                quote! {
                    #enum_name::#vname { #(#names: #ids),* } => false #(|| #tests)*
                }
            }
        }
    });

    quote! {
        match self {
            #(#arms),*,
        }
    }
}

// helpers

fn add_bounds(mut generics: syn::Generics, trait_id: impl quote::ToTokens) -> syn::Generics {
    let params = generics.params.clone();
    let where_clause = generics.make_where_clause();

    for param in &params {
        if let syn::GenericParam::Type(type_param) = param {
            let ident = &type_param.ident;

            where_clause.predicates.push(syn::parse_quote! {
                #ident: #trait_id
            });
        }
    }

    generics
}

fn has_attr(attrs: &Vec<Attribute>, path: &str, name: &str) -> bool {
    attrs.iter().any(|attr| {
        attr.path().is_ident(path)
            && attr.parse_args::<syn::Ident>().map_or(false, |ident| ident == name)
    })
}

fn has_bool_attr(attrs: &Vec<Attribute>, path: &str) -> Option<syn::LitBool> {
    attrs.iter().find_map(|attr| {
        if attr.path().is_ident(path) {
            attr.parse_args::<syn::LitBool>().ok()
        } else {
            None
        }
    })
}
