use std::ops::RangeInclusive;

use kw::{ANY, EOI};
use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Ident, LitChar, LitStr, Token, parenthesized,
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

#[derive(Debug)]
enum Term {
    AnyChar,
    Choice(Vec<Term>),
    EOI,
    Literal(String),
    NegLookahead(Box<Term>),
    Optional(Box<Term>),
    Plus(Box<Term>),
    PosLookahead(Box<Term>),
    Range(RangeInclusive<char>),
    Rule(Ident),
    Sequence(Vec<Term>),
    Star(Box<Term>),
}

mod kw {
    syn::custom_keyword!(ANY);
    syn::custom_keyword!(EOI);
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
        fn parse_range(input: ParseStream) -> syn::Result<Term> {
            let start = input.parse::<LitChar>()?.value();
            input.parse::<Token![..]>()?;
            let end = input.parse::<LitChar>()?.value();
            Ok(Term::Range(start..=end))
        }

        fn parse_atom(input: ParseStream) -> syn::Result<Term> {
            let look = input.lookahead1();
            if look.peek(Ident) {
                if input.parse::<ANY>().is_ok() {
                    Ok(Term::AnyChar)
                } else if input.parse::<EOI>().is_ok() {
                    Ok(Term::EOI)
                } else {
                    input.parse().map(Term::Rule)
                }
            } else if look.peek(LitStr) {
                input.parse::<LitStr>().map(|x| Term::Literal(x.value()))
            } else if look.peek(LitChar) {
                parse_range(input)
            } else if look.peek(syn::token::Paren) {
                let content;
                parenthesized!(content in input);
                parse_choice(&content)
            } else {
                Err(look.error())
            }
        }

        fn parse_repeat(input: ParseStream) -> syn::Result<Term> {
            let mut result = parse_atom(input)?;
            loop {
                if input.parse::<Token![?]>().is_ok() {
                    result = Term::Optional(Box::new(result));
                } else if input.parse::<Token![+]>().is_ok() {
                    result = Term::Plus(Box::new(result));
                } else if input.parse::<Token![*]>().is_ok() {
                    result = Term::Star(Box::new(result));
                } else {
                    break;
                }
            }
            Ok(result)
        }

        fn parse_lookahead(input: ParseStream) -> syn::Result<Term> {
            if input.parse::<Token![!]>().is_ok() {
                parse_repeat(input).map(|x| Term::NegLookahead(x.into()))
            } else if input.parse::<Token![&]>().is_ok() {
                parse_repeat(input).map(|x| Term::PosLookahead(x.into()))
            } else {
                parse_repeat(input)
            }
        }

        fn parse_sequence(input: ParseStream) -> syn::Result<Term> {
            let mut terms = vec![parse_lookahead(input)?];
            while !input.is_empty() && !input.peek(Token![/]) && !input.peek(Token![;]) {
                terms.push(parse_lookahead(input)?);
            }
            if terms.len() == 1 {
                Ok(terms.pop().unwrap())
            } else {
                Ok(Term::Sequence(terms))
            }
        }

        fn parse_choice(input: ParseStream) -> syn::Result<Term> {
            let mut choices = vec![parse_sequence(input)?];
            while input.peek(Token![/]) {
                input.parse::<Token![/]>()?;
                choices.push(parse_sequence(input)?);
            }
            if choices.len() == 1 {
                Ok(choices.pop().unwrap())
            } else {
                Ok(Term::Choice(choices))
            }
        }

        parse_choice(input)
    }
}

impl Term {
    fn generate_code(&self) -> proc_macro2::TokenStream {
        match self {
            Term::AnyChar => quote! {
                p.any()
            },
            Term::EOI => quote! {
                p.eoi()
            },
            Term::Rule(ident) => quote! {
                #ident(p)
            },
            Term::Literal(lit_str) => quote! {
                p.literal(#lit_str)
            },
            Term::Sequence(terms) => {
                let expr = terms
                    .iter()
                    .map(|t| t.generate_code())
                    .reduce(|x, y| quote! { #x && #y })
                    .unwrap();
                quote! {
                    {
                        let save = p.save();
                        if #expr {
                            true
                        } else {
                            p.restore(save);
                            false
                        }
                    }
                }
            }
            Term::Choice(terms) => {
                let code = terms
                    .iter()
                    .map(|t| t.generate_code())
                    .reduce(|x, y| quote! { #x || #y })
                    .unwrap();
                quote! {
                    ( #code )
                }
            }
            Term::Optional(term) => {
                let expr = term.generate_code();
                quote! {
                    ( #expr || true )
                }
            }
            Term::Star(term) => {
                let expr = term.generate_code();
                quote! {
                    { while #expr {}; true }
                }
            }
            Term::Plus(term) => {
                let expr = term.generate_code();
                quote! {
                    {
                        let mut closure = || #expr;
                        if !closure() {
                            return false;
                        }
                        while closure() {}
                        true
                    }
                }
            }
            Term::Range(range) => {
                let (lo, hi) = (range.start(), range.end());
                quote! {
                    p.range(#lo..=#hi)
                }
            }
            Term::NegLookahead(term) => {
                let code = term.generate_code();
                quote! {
                    {
                        let save = p.save();
                        if #code {
                            p.restore(save);
                            false
                        } else {
                            true
                        }
                    }
                }
            }
            Term::PosLookahead(term) => {
                let code = term.generate_code();
                quote! {
                    {
                        let save = p.save();
                        if #code {
                            p.restore(save);
                            true
                        } else {
                            false
                        }
                    }
                }
            }
        }
    }
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
