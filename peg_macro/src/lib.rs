use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Ident, LitStr, Token,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
};

struct Grammar {
    rules: Punctuated<Rule, Token![;]>,
}

struct Rule {
    name: Ident,
    definition: Term,
}

enum Term {
    Literal(LitStr),
    Rule(Ident),
    Sequence(Vec<Term>),
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
        let name = input.parse()?;
        input.parse::<Token![=]>()?;
        let definition = input.parse()?;
        Ok(Self { name, definition })
    }
}

impl Parse for Term {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut terms = vec![];
        let parse_one = || {
            let look = input.lookahead1();
            if look.peek(Ident) {
                input.parse().map(Term::Rule)
            } else if look.peek(LitStr) {
                input.parse().map(Term::Literal)
            } else {
                Err(look.error())
            }
        };
        terms.push(parse_one()?);
        while !input.is_empty() && !input.peek(Token![;]) {
            terms.push(parse_one()?);
        }
        if terms.len() == 1 {
            Ok(terms.pop().unwrap())
        } else {
            Ok(Term::Sequence(terms))
        }
    }
}

impl Term {
    fn generate_code(&self) -> proc_macro2::TokenStream {
        match self {
            Term::Rule(ident) => quote! {
                println!("call subrule");
                // call subrule
            },
            Term::Literal(lit_str) => quote! {
                println!("parse literal");
                // call p.literal()
            },
            Term::Sequence(terms) => quote! {
                println!("sequence");
            },
        }
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

            // Generate a string for debugging
            fn term2str(t: &Term) -> String {
                match t {
                    Term::Rule(ident) => format!("rule {}", ident.to_string()),
                    Term::Literal(lit_str) => format!("literal \"{}\"", lit_str.value()),
                    Term::Sequence(terms) => {
                        let parts: Vec<_> = terms.iter().map(term2str).collect();
                        format!("sequence [{}]", parts.join(", "))
                    }
                }
            }
            let text = format!("I'm generated! ({})", term2str(&r.definition));

            let generated = r.definition.generate_code();
            quote! {
                fn #fn_name(p: &mut crate::peg::ParseState) -> String {
                    #generated
                    #text.into()
                }
            }
        })
        .collect();
    quote! {
        #(#fns)*
    }
    .into()
}
