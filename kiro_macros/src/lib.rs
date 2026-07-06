use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn kiro_export(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[proc_macro_attribute]
pub fn kiro_handle(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[proc_macro_attribute]
pub fn kiro_struct(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}
