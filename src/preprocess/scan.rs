/*
    What are we parsing?

    1. Evaluate macro invocations first.
        - Resolve escape sequences, e.g., long lines.
        - Replace top-level invocations with their output.
        - Eventually, we'll replace in-line `${...}` expressions.
        - Output: a string representing a ZZT-OOP program.
    2. Parse ZZT-OOP next.
        - Output: looks like Vec<Statement>.
        - Statements are ZZT-OOP statements: text, centered, command, move, etc.

    From there, we can implement things like label rewriting.
*/

use std::{
    iter::{Fuse, Peekable},
    str::CharIndices,
};

#[derive(Debug, Eq, PartialEq)]
pub enum Token {
    Identifier(String),
    Newline,
    Percent,
    RawText(String),
    String(String),
}

pub fn scan(input: &str) -> (Vec<Token>, Vec<String>) {
    let mut scanner = Scanner::new(input);
    scanner.scan();
    (scanner.tokens, scanner.errors)
}

struct Scanner<'a> {
    text: Peekable<Fuse<CharIndices<'a>>>,
    tokens: Vec<Token>,
    errors: Vec<String>,
}

impl<'a> Scanner<'a> {
    fn new(input: &'a str) -> Self {
        Scanner {
            text: input.char_indices().fuse().peekable(),
            tokens: vec![],
            errors: vec![],
        }
    }

    fn scan(&mut self) {
        while let Some(next) = self.peek() {
            match next {
                '\n' => {
                    self.tokens.push(Token::Newline);
                    self.advance();
                }
                '%' => {
                    self.tokens.push(Token::Percent);
                    self.advance();
                    self.scan_macro()
                }
                _ => self.raw_text(),
            }
        }
    }

    fn scan_macro(&mut self) {
        while let Some(next) = self.peek() {
            match next {
                '\n' => break,
                ' ' => {
                    self.advance();
                }
                '"' => {
                    self.string();
                }
                'A'..='Z' | 'a'..='z' | '_' => {
                    self.identifier();
                }
                _ => {
                    if let Some((i, c)) = self.text.peek() {
                        self.errors
                            .push(format!("Unexpected character {} at offset {}", c, i));
                    }
                    self.advance();
                }
            }
        }
    }

    fn string(&mut self) {
        self.advance(); // discard leading quote
        let mut result = String::new();
        while let Some(next) = self.peek() {
            match next {
                '"' => break,
                '\\' => {
                    self.advance();
                    let char = self.get_escaped_char();
                    if char != '\n' {
                        result.push(char)
                    }
                }
                _ => {
                    result.push(next);
                    self.advance();
                }
            }
        }
        if !self.consume('"') {
            self.errors.push("string not terminated".into());
        }
        self.tokens.push(Token::String(result));
    }

    fn identifier(&mut self) {
        let mut result = String::new();
        while let Some(next) = self.peek() {
            match next {
                '0'..='9' | 'A'..='Z' | 'a'..='z' | '_' => {
                    result.push(next);
                    self.advance();
                }
                _ => break,
            }
        }
        self.tokens.push(Token::Identifier(result));
    }

    fn raw_text(&mut self) {
        let mut result = String::new();
        while let Some(next) = self.peek() {
            match next {
                '\n' => break,
                '\\' => {
                    self.advance();
                    let char = self.get_escaped_char();
                    if char != '\n' {
                        result.push(char);
                    }
                }
                _ => {
                    result.push(next);
                    self.advance();
                }
            }
        }
        self.tokens.push(Token::RawText(result));
    }

    fn get_escaped_char(&mut self) -> char {
        if let Some(next) = self.advance() {
            next
        } else {
            '\\'
        }
    }

    fn peek(&mut self) -> Option<char> {
        self.text.peek().map(|(_, c)| *c)
    }

    fn advance(&mut self) -> Option<char> {
        self.text.next().map(|(_, c)| c)
    }

    fn consume(&mut self, c: char) -> bool {
        if let Some(next) = self.peek() {
            if next == c {
                self.advance();
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use std::vec;

    use super::*;
    use Token::*;
    macro_rules! token_helper {
        ($fn:ident, $type:ident) => {
            fn $fn(s: &str) -> Token {
                Token::$type(s.to_string())
            }
        };
    }
    token_helper!(identifier, Identifier);
    token_helper!(raw_text, RawText);
    token_helper!(string, String);

    fn scan(input: &str) -> Vec<Token> {
        super::scan(input).0
    }

    #[test]
    fn scan_empty() {
        assert_eq!(vec![] as Vec<Token>, scan(""));
    }

    #[test]
    fn scan_raw() {
        assert_eq!(vec![raw_text("Hello, world!")], scan("Hello, world!"));
    }

    #[test]
    fn scan_raw_multiline() {
        assert_eq!(
            vec![raw_text("foo"), Newline, raw_text("bar")],
            scan("foo\nbar")
        );
    }

    #[test]
    fn scan_mixed() {
        assert_eq!(
            vec![
                raw_text("foo"),
                Newline,
                Percent,
                identifier("bar"),
                Newline,
                raw_text("baz")
            ],
            scan("foo\n%bar\nbaz")
        );
    }

    #[test]
    fn scan_multiline() {
        assert_eq!(
            vec![raw_text("foo"), Newline, raw_text("bar")],
            scan("foo\nbar")
        );
    }

    #[test]
    fn scan_raw_escapes() {
        assert_eq!(
            vec![raw_text(r#"a long-line of raw text with \escapes"#)],
            scan(concat!(
                r#"a long\-line of raw text with\"#,
                "\n",
                r#" \\escapes"#
            ))
        );
    }

    #[test]
    fn scan_directive() {
        assert_eq!(vec![Percent, identifier("foo")], scan("%foo"));
    }

    #[test]
    fn scan_multiple_identifiers() {
        assert_eq!(
            vec![Percent, identifier("foo"), identifier("bar")],
            scan("%foo bar")
        );
    }

    #[test]
    fn scan_directive_string() {
        assert_eq!(
            vec![Percent, identifier("foo"), string("bar")],
            scan("%foo \"bar\"")
        );
    }

    #[test]
    fn scan_directive_string_escapes() {
        assert_eq!(
            vec![
                Percent,
                identifier("foo"),
                string(r#"a long-line string with "quotes" and \escapes"#)
            ],
            scan(concat!(
                r#"%foo "a long\-line string with\"#,
                "\n",
                r#" \"quotes\" and \\escapes""#
            ))
        );
    }

    #[test]
    fn scan_premature_end_of_string() {
        let (tokens, errors) = super::scan("%foo \"bar");
        assert_eq!(vec![Percent, identifier("foo"), string("bar")], tokens);
        assert!(errors.len() == 1);
    }

    #[test]
    fn scan_unexpected_chars() {
        let (tokens, errors) = super::scan("%foo @bar");
        assert_eq!(vec![Percent, identifier("foo"), identifier("bar")], tokens);
        assert!(errors.len() == 1);
    }
}
