use anyhow::{anyhow, Result};

use super::scan::Token;

pub fn parse(input: Vec<Token>) -> Result<Vec<Expr>> {
    // TODO: Should this take ownership of the tokens?
    let mut parser = Parser::new(input);
    parser.parse()
}

struct Parser {
    pos: usize,
    tokens: Vec<Token>,
}

#[derive(Debug)]
pub enum Expr {
    ZztOop(String),
    Macro(String, Vec<Box<Expr>>),
    String(String),
}

impl Parser {
    fn new(input: Vec<Token>) -> Self {
        Self {
            pos: 0,
            tokens: input,
        }
    }

    fn parse(&mut self) -> Result<Vec<Expr>> {
        let mut result = vec![];
        while let Some(token) = self.peek() {
            match token {
                Token::Newline | Token::RawText(_) => result.push(self.parse_zzt_oop()),
                Token::Percent => result.push(self.parse_macro()?),
                _ => return Err(anyhow!("Unexpected token {:?}", token)),
            }
        }
        Ok(result)
    }

    fn parse_zzt_oop(&mut self) -> Expr {
        let mut parts: Vec<String> = vec![];
        while let Some(token) = self.peek() {
            match token {
                Token::RawText(s) => {
                    parts.push(s.into());
                    self.advance();
                }
                Token::Newline => {
                    parts.push("\n".into());
                    self.advance();
                }
                _ => break,
            }
        }
        Expr::ZztOop(parts.concat())
    }

    fn parse_macro(&mut self) -> anyhow::Result<Expr> {
        self.advance().unwrap(); // consume %

        let ident = self.advance();
        let name = match ident {
            Some(Token::Identifier(name)) => name.into(),
            _ => return Err(anyhow!("Expected identifier")),
        };

        let mut args = vec![];
        while let Some(token) = self.peek() {
            match token {
                Token::String(s) => {
                    let val = s.into();
                    self.advance();
                    args.push(Box::new(Expr::String(val)));
                }
                Token::Newline => {
                    self.advance();
                    break;
                }
                _ => return Err(anyhow!("Unexpected token {:?}", token)),
            }
        }
        Ok(Expr::Macro(name, args))
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<&Token> {
        let result = self.tokens.get(self.pos);
        self.pos = (self.pos + 1).min(self.tokens.len());
        result
    }
    // TODO: a consume helper that takes a list of expr variants?
}

#[cfg(test)]
mod tests {
    use crate::preprocess::scan::scan;
    use insta::assert_debug_snapshot;

    use super::*;

    fn parse_str(input: &str) -> Vec<Expr> {
        let tokens = scan(input).0;
        parse(tokens).expect("Failed to parse code string")
    }

    #[test]
    fn raw_text() {
        assert_debug_snapshot!(parse_str("This is\nsome text."), @r###"
        [
            ZztOop(
                "This is\nsome text.",
            ),
        ]
        "###);
    }

    #[test]
    fn single_macro() {
        assert_debug_snapshot!(parse_str("%foo"), @r###"
        [
            Macro(
                "foo",
                [],
            ),
        ]
        "###);
    }

    #[test]
    fn macro_with_args() {
        assert_debug_snapshot!(parse_str(r#"%foo "bar" "baz""#), @r###"
        [
            Macro(
                "foo",
                [
                    String(
                        "bar",
                    ),
                    String(
                        "baz",
                    ),
                ],
            ),
        ]
        "###)
    }

    #[test]
    fn macro_with_literals() {
        assert_debug_snapshot!(parse_str("Foo.\n%bar\nBaz."), @r###"
        [
            ZztOop(
                "Foo.\n",
            ),
            Macro(
                "bar",
                [],
            ),
            ZztOop(
                "Baz.",
            ),
        ]
        "###)
    }
}
