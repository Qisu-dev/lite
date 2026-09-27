mod symbols;

use proc_macro::TokenStream;
use symbols::symbols_impl;

#[proc_macro]
pub fn symbols(input: TokenStream) -> TokenStream {
    symbols_impl(input)
}
