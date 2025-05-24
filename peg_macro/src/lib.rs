use proc_macro::TokenStream;
use quote::quote;

#[proc_macro]
pub fn grammar(ts: TokenStream) -> TokenStream {
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
