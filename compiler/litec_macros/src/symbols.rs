use heck::ToSnakeCase;
use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::quote;
use syn::{
    LitStr, Token, braced,
    parse::{Parse, ParseStream},
};

struct SymbolDef {
    name: Ident,
    value: Option<String>,
}

impl Parse for SymbolDef {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        let value = if input.peek(Token![:]) {
            input.parse::<Token![:]>()?;
            let lit: LitStr = input.parse()?;
            Some(lit.value())
        } else {
            None
        };
        Ok(SymbolDef { name, value })
    }
}

struct Group {
    name: Ident,
    symbols: Vec<SymbolDef>,
}

impl Parse for Group {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        let content;
        braced!(content in input);
        let symbols = content.parse_terminated(SymbolDef::parse, Token![,])?;
        Ok(Group {
            name,
            symbols: symbols.into_iter().collect(),
        })
    }
}

struct SymbolsInput {
    groups: Vec<Group>,
}

impl Parse for SymbolsInput {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut groups = Vec::new();
        while !input.is_empty() {
            groups.push(input.parse()?);
        }
        Ok(SymbolsInput { groups })
    }
}

pub(crate) fn symbols_impl(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as SymbolsInput);

    let mut global_idx = 0u32;
    let mut all_names = Vec::new();
    let mut all_symbols = Vec::new();
    let mut module_defs = Vec::new();

    for group in input.groups {
        let group_name = Ident::new(
            format!("{}_generated", group.name.to_string().to_snake_case()).as_str(),
            group.name.span(),
        );

        let mut const_defs = Vec::new();
        let mut symbol_names = Vec::new();
        let mut mod_symbols = Vec::new();

        for sym in group.symbols.into_iter() {
            let ident = &sym.name;
            let idx = global_idx;

            const_defs.push(quote! {
                pub const #ident: crate::Symbol = crate::Symbol::new(#idx);
            });

            mod_symbols.push(quote! { #ident });

            let symbal_name = sym.value.unwrap_or_else(|| ident.to_string());
            symbol_names.push(quote! { #symbal_name });

            all_names.push(quote! { #symbal_name });
            all_symbols.push(quote! { crate::Symbol::new(#idx) });

            global_idx += 1;
        }

        let strs_name = Ident::new(
            format!("{}_STRS", group.name.to_string().to_uppercase()).as_str(),
            group.name.span(),
        );
        let symbols_name = Ident::new(
            format!("{}_SYMBOLS", group.name.to_string().to_uppercase()).as_str(),
            group.name.span(),
        );

        module_defs.push(quote! {
            #[allow(non_upper_case_globals)]
            pub mod #group_name {
                use crate::Symbol;
                #(#const_defs)*

                pub const #strs_name: &[&str] = &[ #(#symbol_names),* ];
                pub const #symbols_name: &[Symbol] = &[ #(#mod_symbols),* ];
            }
        });
    }

    let expanded = quote! {
        pub mod sym {
            #(#module_defs)*
        }

        pub const ALL_NAMES: &[&str] = &[ #(#all_names),* ];
        pub const ALL_SYMBOLS: &[Symbol] = &[ #(#all_symbols),* ];
    };

    expanded.into()
}

