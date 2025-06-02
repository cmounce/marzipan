use std::{
    num::NonZero,
    ops::{Range, RangeInclusive},
};

pub struct ParseState<T: Clone> {
    pub input: String,
    pub offset: usize,
    captures: Vec<RawCapture<T>>,
}

pub struct Savepoint {
    offset: usize,
}

pub struct Captures<'a, T: Clone> {
    input: &'a str,
    raw: &'a [RawCapture<T>],
    index: usize,
}

pub struct Capture<'a, T: Clone> {
    input: &'a str,
    raw: &'a [RawCapture<T>],
}

#[derive(Debug)]
struct RawCapture<T: Clone> {
    kind: T,
    span: Range<usize>,
    subtree_len: Option<NonZero<usize>>,
}

impl<T: Clone> ParseState<T> {
    pub fn new(s: &str) -> Self {
        Self {
            input: s.into(),
            offset: 0,
            captures: vec![],
        }
    }

    pub fn save(&self) -> Savepoint {
        Savepoint {
            offset: self.offset,
        }
    }

    pub fn restore(&mut self, save: Savepoint) {
        self.offset = save.offset;
    }

    pub fn literal(&mut self, s: &str) -> bool {
        if self.input[self.offset..].starts_with(s) {
            self.offset += s.len();
            true
        } else {
            false
        }
    }

    pub fn literal_i(&mut self, s: &str) -> bool {
        if self.input[self.offset..(self.offset + s.len())].eq_ignore_ascii_case(s) {
            self.offset += s.len();
            true
        } else {
            false
        }
    }

    pub fn range(&mut self, r: RangeInclusive<char>) -> bool {
        if let Some(next) = self.input[self.offset..].chars().next() {
            if r.contains(&next) {
                self.offset += next.len_utf8();
                return true;
            }
        }
        false
    }

    pub fn range_i(&mut self, r: RangeInclusive<char>) -> bool {
        if let Some(next) = self.input[self.offset..].chars().next() {
            if r.contains(&next.to_ascii_lowercase()) || r.contains(&next.to_ascii_uppercase()) {
                self.offset += next.len_utf8();
                return true;
            }
        }
        false
    }

    pub fn any(&mut self) -> bool {
        if let Some(c) = self.input[self.offset..].chars().next() {
            self.offset += c.len_utf8();
            true
        } else {
            false
        }
    }

    pub fn eoi(&self) -> bool {
        self.offset >= self.input.len()
    }

    pub fn captures<'a>(&'a self) -> Captures<'a, T> {
        Captures {
            input: &self.input,
            raw: &self.captures,
            index: 0,
        }
    }
}

impl<'a, T: Clone> Iterator for Captures<'a, T> {
    type Item = Capture<'a, T>;

    fn next(&mut self) -> Option<Self::Item> {
        let head = self.raw.get(self.index)?;
        let subtree_len = head.subtree_len.unwrap().get();
        let subtree_slice = &self.raw[self.index..self.index + subtree_len];
        self.index += subtree_len;
        Some(Capture {
            input: &self.input,
            raw: subtree_slice,
        })
    }
}

impl<'a, T: Clone> Capture<'a, T> {
    pub fn children(&self) -> Captures<'a, T> {
        Captures {
            input: &self.input,
            raw: &self.raw[1..],
            index: 0,
        }
    }

    pub fn kind(&self) -> T {
        self.raw[0].kind.clone()
    }

    pub fn span(&self) -> Range<usize> {
        self.raw[0].span.clone()
    }

    pub fn text(&self) -> &'a str {
        &self.input[self.span()]
    }
}

#[cfg(test)]
mod tests {
    use peg_macro::grammar;

    use super::*;

    grammar! {
        fake_csv = line "\n" line EOI;
        line = item "," item;
        item = "foo" / "bar";

        option = "(" "x"? ")";
        plus = "(" "x"+ ")";
        star = "(" "x"* ")";

        word_head = "_" / 'A'..'Z' / 'a'..'z';
        word_tail = word_head / '0'..'9';
        plain_word = word_head word_tail*;
        word = "(" plain_word ")";

        words = "(" (plain_word ("," plain_word)*)? ")";
        nested_choice = "(" ("a" / "b") ("c" / "d") ")";

        dq = "\"";
        quoted = dq ("\\" ANY / !dq ANY)* dq;

        case_sensitivity = "Strict " ('a'..'z')+ ", loose "i ('a'..'z'i)+;

        @icase
        hex_config = "let " var_name " = 0x" ('a'..'f' / '0'..'9')+;
        var_name = "foo" / "bar"; // case must match
    }

    fn parse<C: Clone, T: Fn(&mut ParseState<C>) -> bool>(rule: T, s: &str) -> bool {
        println!("About to parse: {}", s);
        let mut p = ParseState::new(s);
        rule(&mut p) && p.eoi()
    }

    #[test]
    fn test_generated() {
        let mut p = ParseState::new("foo,bar\nbar,foo");
        assert!(fake_csv(&mut p));
        let mut p = ParseState::new("foo,foo\nfoo;foo");
        assert!(!fake_csv(&mut p));
        let mut p = ParseState::new("foo,foo\nfoo,foo\njunk at end");
        assert!(!fake_csv(&mut p));
    }

    #[test]
    fn test_repetition_suffixes() {
        let zero = "()";
        assert!(parse(option, zero));
        assert!(parse(star, zero));
        assert!(!parse(plus, zero));

        let one = "(x)";
        assert!(parse(option, one));
        assert!(parse(star, one));
        assert!(parse(plus, one));

        let many = "(xxx)";
        assert!(!parse(option, many));
        assert!(parse(star, many));
        assert!(parse(plus, many));
    }

    #[test]
    fn test_ranges() {
        assert!(parse(word, "(_FooBar)"));
        assert!(parse(word, "(abc123)"));
        assert!(!parse(word, "(123abc)"));
        assert!(!parse(word, "(foo-bar)"));
    }

    #[test]
    fn test_groups() {
        assert!(parse(words, "()"));
        assert!(parse(words, "(foo)"));
        assert!(parse(words, "(foo,bar,baz)"));
        assert!(!parse(words, "(foo,)"));
        assert!(!parse(words, "(,bar)"));
        assert!(!parse(words, "(3baz)"));
    }

    #[test]
    fn test_nested_choice() {
        assert!(parse(nested_choice, "(ac)"));
        assert!(parse(nested_choice, "(ad)"));
        assert!(parse(nested_choice, "(bc)"));
        assert!(parse(nested_choice, "(bd)"));
        assert!(!parse(nested_choice, "(a)"));
        assert!(!parse(nested_choice, "(b)"));
        assert!(!parse(nested_choice, "(c)"));
        assert!(!parse(nested_choice, "(d)"));
    }

    #[test]
    fn test_quoted() {
        assert!(parse(quoted, r#""""#));
        assert!(parse(quoted, r#""foo""#));
        assert!(parse(quoted, r#""\"""#));
        assert!(parse(quoted, r#""foo \"bar\" baz""#));
        assert!(parse(quoted, r#""C:\\>""#));

        assert!(!parse(quoted, r#""no end"#));
        assert!(!parse(quoted, r#""foo " bar""#));
        assert!(!parse(quoted, r#""false end \""#));
    }

    #[test]
    fn test_i_suffix() {
        assert!(parse(case_sensitivity, "Strict abc, loose xyz"));
        assert!(!parse(case_sensitivity, "STRICT abc, loose xyz"));
        assert!(!parse(case_sensitivity, "Strict ABC, loose xyz"));
        assert!(parse(case_sensitivity, "Strict abc, LOOSE xyz"));
        assert!(parse(case_sensitivity, "Strict abc, loose XYZ"));
        assert!(parse(case_sensitivity, "Strict abc, LoOsE XyZ"));
    }

    #[test]
    fn test_icase_decorator() {
        assert!(parse(hex_config, "let foo = 0xc0ffee"));
        assert!(parse(hex_config, "LET bar = 0XCAFE"));
        assert!(!parse(hex_config, "let Foo = 0xc0ffee"));
        assert!(!parse(hex_config, "let BAR = 0xcafe"));
    }
}
