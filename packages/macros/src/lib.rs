use heck::ToKebabCase;
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(MsgId)]
pub fn derive_msg_id(item: TokenStream) -> TokenStream {
    let item: DeriveInput = parse_macro_input!(item);
    let ident = item.ident;
    let id = ident.to_string().to_kebab_case();

    quote!(
        impl ::arpy::transport::MsgId for #ident {
            const ID: &'static str = #id;
        }
    )
    .into()
}
