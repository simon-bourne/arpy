// TODO: Split this into it's own crate
use std::{collections::HashSet, env, fmt::Display, fs, path::Path};

use itertools::Itertools;
use proc_macro::TokenStream;
use proc_macro2::Span;
use proc_macro_error::{abort, abort_call_site, proc_macro_error};
use quote::quote;
use ra_ap_syntax::{
    ast::{self, HasModuleItem, HasName},
    SourceFile, TokenText,
};
use syn::{
    bracketed,
    parse::{Parse, ParseStream},
    parse_macro_input,
    token::Comma,
    Ident, LitStr,
};

struct FunctionBodyArgs {
    file: LitStr,
    function_body: Ident,
    supporting_fns: Vec<Ident>,
}

impl Parse for FunctionBodyArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let file = input.parse()?;
        input.parse::<Comma>()?;
        let function_body = input.parse()?;
        input.parse::<Comma>()?;
        let supporting_fns;
        bracketed!(supporting_fns in input);
        let supporting_fns = supporting_fns
            .parse_terminated::<Ident, Comma>(Ident::parse)?
            .into_iter()
            .collect();

        Ok(Self {
            file,
            function_body,
            supporting_fns,
        })
    }
}

#[proc_macro]
#[proc_macro_error]
pub fn function_body(input: TokenStream) -> TokenStream {
    let args: FunctionBodyArgs = parse_macro_input!(input);
    let function_body = args.function_body.to_string();
    let mut supporting_fns = HashSet::new();

    supporting_fns.extend(args.supporting_fns.iter().map(Ident::to_string));

    if supporting_fns.contains(&function_body) {
        abort_call_site!("Function body can't be in list of supporting functions");
    }

    let doc = doc_function_body(args.file, args.function_body, Some(&mut supporting_fns));

    if !supporting_fns.is_empty() {
        abort_call_site!(
            "Functions not found: [{}]",
            format!("`{}`", supporting_fns.into_iter().format(", "))
        );
    }

    doc
}

fn doc_function_body(
    file: LitStr,
    function_body_ident: Ident,
    mut supporting_fns: Option<&mut HashSet<String>>,
) -> TokenStream {
    let source = parse_file(&file);

    let mut found_body = false;
    let function_body = function_body_ident.to_string();
    let supporting_fns = &mut supporting_fns;

    // TODO: Don't remove top level comments
    let parts = source.items().filter_map(|item| match item {
        ast::Item::Use(use_item) => Some(hide_in_doc(use_item)),
        ast::Item::Fn(function) => function.name().and_then(|name| {
            let name = name.text();

            if name.as_str() == function_body {
                found_body = true;
                extract_function_body(&function)
            } else if include_function(&name, supporting_fns) {
                Some(format!("{function}\n"))
            } else {
                None
            }
        }),
        other_item => Some(format!("{other_item}\n")),
    });

    let doc = parts.collect::<Vec<String>>().join("\n");

    if !found_body {
        abort!(function_body_ident, "{} not found", function_body);
    }

    quote!(#doc).into()
}

fn include_function(name: &TokenText, supporting_fns: &mut Option<&mut HashSet<String>>) -> bool {
    supporting_fns.as_mut().map(|fns| fns.remove(name.as_str())) != Some(false)
}

fn extract_function_body(function: &ast::Fn) -> Option<String> {
    function.body().map(|body| {
        remove_indent(
            body.to_string()
                .as_str()
                .trim()
                .trim_start_matches('{')
                .trim_end_matches('}'),
        ) + "\n"
    })
}

fn remove_indent(text: &str) -> String {
    let min_indent = text.lines().filter_map(indent).min().unwrap_or(0);

    text.lines()
        .map(|line| {
            if line.len() > min_indent {
                &line[min_indent..]
            } else {
                ""
            }
        })
        .join("\n")
        .trim_matches('\n')
        .to_string()
}

fn indent(text: &str) -> Option<usize> {
    if text.trim().is_empty() {
        None
    } else {
        text.find(|c: char| c != ' ' && c != '\t')
    }
}

#[proc_macro]
#[proc_macro_error]
pub fn file(input: TokenStream) -> TokenStream {
    let file: LitStr = parse_macro_input!(input);

    doc_function_body(file, Ident::new("main", Span::call_site()), None)
}

fn parse_file(file_expr: &LitStr) -> SourceFile {
    let file = file_expr.value();

    let dir = env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|e| abort_call_site!(e));
    let path = Path::new(&dir).join(file);
    let source_code = fs::read_to_string(path).unwrap_or_else(|e| abort!(file_expr, e));
    let parse = SourceFile::parse(&source_code);
    let source = parse.tree();

    if !parse.errors().is_empty() {
        abort!(file_expr, "Errors in source file");
    }

    source
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
