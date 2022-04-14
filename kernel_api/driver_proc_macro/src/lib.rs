use std::str::FromStr;

use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn server_entry(_attr: TokenStream, tokens: TokenStream) -> TokenStream {
    TokenStream::from_str(format!("#[link_name = \"_start\"]\npub extern {}", tokens.to_string()).as_str()).unwrap()
}
