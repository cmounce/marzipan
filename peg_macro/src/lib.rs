use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Ident, Token,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
};

struct Grammar {
    rules: Punctuated<Rule, Token![;]>,
}

struct Rule {
    name: Ident,
}

impl Parse for Grammar {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Grammar {
            rules: Punctuated::parse_terminated(input)?,
        })
    }
}

impl Parse for Rule {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let result = Self {
            name: input.parse()?,
        };
        Ok(result)
    }
}

#[proc_macro]
pub fn grammar_old(ts: TokenStream) -> TokenStream {
    let token = ts.into_iter().next().expect("must have at least one token");
    let result = match token {
        proc_macro::TokenTree::Literal(literal) => {
            let s = literal.to_string();
            let c = s.chars().next().unwrap_or('\0');
            match c {
                '0'..'9' => {
                    let val: usize = s.parse().unwrap();
                    quote! { Some(crate::peg::Foo::Bar(#val)) }
                }
                '"' => {
                    let inner = &s[1..s.len() - 1];
                    quote! { Some(crate::peg::Foo::Baz(#inner.into())) }
                }
                _ => quote! { None },
            }
        }
        _ => quote! { None },
    };

    quote! { println!("Hello, world! {:?}", #result); }.into()
}

#[proc_macro]
pub fn grammar(ts: TokenStream) -> TokenStream {
    let input = parse_macro_input!(ts as Grammar);
    let fns: Vec<_> = input
        .rules
        .iter()
        .map(|r| {
            let fn_name = &r.name;
            quote! {
                fn #fn_name() -> String {
                    "I'm a generated function!".into()
                }
            }
        })
        .collect();
    quote! {
        #(#fns)*
    }
    .into()
}
