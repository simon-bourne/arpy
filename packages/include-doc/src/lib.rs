// TODO: Split this into it's own crate
use std::{env, fmt::Display, fs, path::Path};

use itertools::Itertools;
use proc_macro::TokenStream;
use proc_macro_error::{abort, abort_call_site, proc_macro_error};
use quote::quote;
use ra_ap_syntax::{
    ast::{self, HasModuleItem},
    SourceFile,
};
use syn::{parse_macro_input, LitStr};

// TODO: `fn function_body` that includes any global items, the function body
// and maybe a list of supporting functions.

#[proc_macro]
#[proc_macro_error]
pub fn file(input: TokenStream) -> TokenStream {
    let file_expr: LitStr = parse_macro_input!(input);
    let file = file_expr.value();

    let dir = env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|e| abort_call_site!(e));
    let path = Path::new(&dir).join(file);
    let source_code = fs::read_to_string(path).unwrap_or_else(|e| abort!(file_expr, e));
    let parse = SourceFile::parse(&source_code);
    let source = parse.tree();

    if !parse.errors().is_empty() {
        abort!(file_expr, "Errors in source file");
    }

    // TODO: Don't remove top level comments
    let parts = source.items().map(|item| match item {
        ast::Item::Use(use_item) => hide_in_doc(use_item),
        other_item => format!("{other_item}\n"),
    });

    let doc = parts.collect::<Vec<String>>().join("\n");

    quote!(#doc).into()
}

fn hide_in_doc(item: impl Display) -> String {
    // We need the extra `"\n#"` as otherwise rustdoc won't include attributes after
    // hidden items. e.g.
    //
    // ```
    // # use blah
    // #[attribute_will_also_be_hidden]
    // ```
    format!("# {}\n", item.to_string().lines().format("")) + "#"
}
