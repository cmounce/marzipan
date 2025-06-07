use std::ops::RangeInclusive;

use kw::{ANY, EOI};
use proc_macro::TokenStream;
use proc_macro2::Span;
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
    Capture(String, Box<Term>),
    Choice(Vec<Term>),
    EOI,
    Literal(String, bool),
    NegLookahead(Box<Term>),
    Optional(Box<Term>),
    Plus(Box<Term>),
    PosLookahead(Box<Term>),
    Range(RangeInclusive<char>, bool),
    Rule(Ident),
    Sequence(Vec<Term>),
    Star(Box<Term>),
}

mod kw {
    syn::custom_keyword!(ANY);
    syn::custom_keyword!(EOI);
    syn::custom_keyword!(icase);
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
        let mut icase = false;
        if input.parse::<Token![@]>().is_ok() {
            let look = input.lookahead1();
            if look.peek(kw::icase) {
                input.parse::<kw::icase>()?;
                icase = true;
            } else {
                return Err(look.error());
            }
        }

        let name = input.parse()?;
        input.parse::<Token![=]>()?;
        let mut definition: Term = input.parse()?;
        if icase {
            definition.set_icase();
        }
        Ok(Self { name, definition })
    }
}

impl Parse for Term {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        fn parse_range(input: ParseStream) -> syn::Result<Term> {
            let start_lit = input.parse::<LitChar>()?;
            input.parse::<Token![..]>()?;
            let end_lit = input.parse::<LitChar>()?;
            let range = start_lit.value()..=end_lit.value();
            let icase = end_lit.suffix() == "i";
            Ok(Term::Range(range, icase))
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
                let lit = input.parse::<LitStr>()?;
                let icase = lit.suffix() == "i";
                Ok(Term::Literal(lit.value(), icase))
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

        fn parse_prefix(input: ParseStream) -> syn::Result<Term> {
            if input.parse::<Token![!]>().is_ok() {
                parse_repeat(input).map(|x| Term::NegLookahead(x.into()))
            } else if input.parse::<Token![&]>().is_ok() {
                parse_repeat(input).map(|x| Term::PosLookahead(x.into()))
            } else if input.parse::<Token![#]>().is_ok() {
                let tag: Ident = input.parse()?;
                input.parse::<Token![:]>()?;
                let expr = parse_repeat(input)?;
                Ok(Term::Capture(tag.to_string(), expr.into()))
            } else {
                parse_repeat(input)
            }
        }

        fn parse_sequence(input: ParseStream) -> syn::Result<Term> {
            let mut terms = vec![parse_prefix(input)?];
            while !input.is_empty() && !input.peek(Token![/]) && !input.peek(Token![;]) {
                terms.push(parse_prefix(input)?);
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
            Term::Capture(name, pat) => {
                let tag = Ident::new(&name, Span::call_site());
                let code = pat.generate_code();
                quote! {
                    {
                        let save = p.begin_capture(Tag::#tag);
                        if !#code {
                            p.restore(save);
                            false
                        } else {
                            p.commit_capture(save);
                            true
                        }
                    }
                }
            }
            Term::EOI => quote! {
                p.eoi()
            },
            Term::Rule(ident) => quote! {
                #ident(p)
            },
            Term::Literal(lit_str, icase) => {
                let method = if *icase {
                    quote! { literal_i }
                } else {
                    quote! { literal }
                };
                quote! {
                    p.#method(#lit_str)
                }
            }
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
                        if closure() {
                            while closure() {}
                            true
                        } else {
                            false
                        }
                    }
                }
            }
            Term::Range(range, icase) => {
                let (lo, hi) = (range.start(), range.end());
                let method = if *icase {
                    quote! { range_i }
                } else {
                    quote! { range }
                };
                quote! {
                    p.#method(#lo..=#hi)
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

    fn set_icase(&mut self) {
        match self {
            Term::Literal(_, icase) | Term::Range(_, icase) => {
                *icase = true;
            }
            Term::Choice(terms) | Term::Sequence(terms) => {
                terms.iter_mut().for_each(|x| x.set_icase());
            }
            Term::Capture(_, term)
            | Term::NegLookahead(term)
            | Term::Optional(term)
            | Term::Plus(term)
            | Term::PosLookahead(term)
            | Term::Star(term) => {
                term.set_icase();
            }
            Term::AnyChar | Term::EOI | Term::Rule(_) => {}
        }
    }

    fn get_capture_names(&self) -> Vec<&str> {
        let mut result = vec![];
        match self {
            Term::AnyChar | Term::EOI | Term::Literal(_, _) | Term::Range(_, _) | Term::Rule(_) => {
            }
            Term::Capture(name, term) => {
                result.push(name.as_str());
                result.extend(term.get_capture_names());
            }
            Term::Choice(terms) | Term::Sequence(terms) => {
                terms
                    .iter()
                    .for_each(|x| result.extend(x.get_capture_names()));
            }
            Term::NegLookahead(term)
            | Term::Optional(term)
            | Term::Plus(term)
            | Term::PosLookahead(term)
            | Term::Star(term) => {
                result.extend(term.get_capture_names());
            }
        }
        result
    }
}

#[proc_macro]
pub fn grammar(ts: TokenStream) -> TokenStream {
    let input = parse_macro_input!(ts as Grammar);

    let mut capture_names: Vec<_> = input
        .rules
        .iter()
        .flat_map(|r| r.definition.get_capture_names())
        .collect();
    capture_names.sort();
    capture_names.dedup();
    let tag_idents: Vec<Ident> = capture_names
        .iter()
        .map(|x| Ident::new(x, proc_macro2::Span::call_site()))
        .collect();
    let enum_tag = quote! {
        #[derive(Copy, Clone, Debug)]
        enum Tag {
            #(#tag_idents),*
        }
    };

    let fns: Vec<_> = input
        .rules
        .iter()
        .map(|r| {
            let fn_name = &r.name;
            let generated = r.definition.generate_code();
            quote! {
                fn #fn_name(p: &mut crate::peg::ParseState<Tag>) -> bool {
                    use crate::peg::backend::LowLevel;
                    #generated
                }
            }
        })
        .collect();
    quote! {
        #enum_tag
        #(#fns)*
    }
    .into()
}
