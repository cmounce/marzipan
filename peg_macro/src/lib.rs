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
    Choice(Vec<Term>),
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
        fn parse_atom(input: ParseStream) -> syn::Result<Term> {
            let look = input.lookahead1();
            if look.peek(Ident) {
                input.parse().map(Term::Rule)
            } else if look.peek(LitStr) {
                input.parse().map(Term::Literal)
            } else {
                Err(look.error())
            }
        }

        fn parse_term(input: ParseStream) -> syn::Result<Term> {
            let mut choices = vec![parse_atom(input)?];
            while input.peek(Token![/]) {
                input.parse::<Token![/]>()?;
                choices.push(parse_atom(input)?);
            }
            if choices.len() == 1 {
                Ok(choices.pop().unwrap())
            } else {
                Ok(Term::Choice(choices))
            }
        }

        fn parse_sequence(input: ParseStream) -> syn::Result<Term> {
            let mut terms = vec![parse_term(input)?];
            while !input.is_empty() && !input.peek(Token![;]) {
                terms.push(parse_term(input)?);
            }
            if terms.len() == 1 {
                Ok(terms.pop().unwrap())
            } else {
                Ok(Term::Sequence(terms))
            }
        }

        parse_sequence(input)
    }
}

impl Term {
    fn generate_code(&self) -> proc_macro2::TokenStream {
        match self {
            Term::Rule(ident) => quote! {
                #ident(p)
            },
            Term::Literal(lit_str) => quote! {
                p.literal(#lit_str)
            },
            Term::Sequence(terms) => {
                let expr = terms
                    .iter()
                    .map(|t| t.generate_wrapped_code())
                    .reduce(|x, y| quote! { #x && #y })
                    .unwrap();
                quote! {
                    let save = p.save();
                    if #expr {
                        true
                    } else {
                        p.restore(save);
                        false
                    }
                }
            }
            Term::Choice(terms) => terms
                .iter()
                .map(|t| t.generate_wrapped_code())
                .reduce(|x, y| quote! { #x || #y })
                .unwrap(),
        }
    }

    fn generate_wrapped_code(&self) -> proc_macro2::TokenStream {
        let code = self.generate_code();
        match self {
            Term::Sequence(_) | Term::Choice(_) => quote! {
                { #code }
            },
            _ => code,
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
            let generated = r.definition.generate_code();
            quote! {
                fn #fn_name(p: &mut crate::peg::ParseState) -> bool {
                    #generated
                }
            }
        })
        .collect();
    quote! {
        #(#fns)*
    }
    .into()
}
