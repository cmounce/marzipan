use std::{
    num::NonZero,
    ops::{Range, RangeInclusive},
};

pub struct Parser {
    input: String,
    offset: usize,
    captures: Vec<RawCapture>,
    case_sensitive: bool,
}

impl Parser {
    pub fn new(input: &str) -> Self {
        Self {
            input: input.into(),
            offset: 0,
            captures: Vec::new(),
            case_sensitive: true,
        }
    }

    pub fn save(&self) -> Savepoint {
        Savepoint {
            offset: self.offset,
            num_captures: self.captures.len(),
        }
    }

    pub fn restore(&mut self, sp: Savepoint) {
        self.offset = sp.offset;
        self.captures.truncate(sp.num_captures);
    }

    pub fn iter<'a>(&'a self) -> Captures<'a> {
        Captures {
            input: &self.input,
            raw: &self.captures,
            index: 0,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Savepoint {
    offset: usize,
    num_captures: usize,
}

pub struct Captures<'a> {
    input: &'a str,
    raw: &'a [RawCapture],
    index: usize,
}

impl<'a> Iterator for Captures<'a> {
    type Item = Capture<'a>;

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

pub struct Capture<'a> {
    input: &'a str,
    raw: &'a [RawCapture],
}

impl<'a> Capture<'a> {
    pub fn children(&self) -> Captures<'a> {
        Captures {
            input: &self.input,
            raw: &self.raw[1..],
            index: 0,
        }
    }

    pub fn kind(&self) -> &'static str {
        &self.raw[0].kind
    }

    pub fn span(&self) -> Range<usize> {
        self.raw[0].span.clone()
    }

    pub fn text(&self) -> &'a str {
        &self.input[self.span()]
    }
}

#[derive(Debug)]
struct RawCapture {
    kind: &'static str,
    span: Range<usize>,
    subtree_len: Option<NonZero<usize>>,
}

pub trait Rule {
    fn parse(&self, p: &mut Parser) -> bool;
}

impl Rule for Box<dyn Rule> {
    fn parse(&self, p: &mut Parser) -> bool {
        self.as_ref().parse(p)
    }
}

pub struct Ref<T>(pub T);

impl<T> Rule for Ref<&T>
where
    T: Rule,
{
    fn parse(&self, p: &mut Parser) -> bool {
        self.0.parse(p)
    }
}

impl<R, F> Rule for F
where
    R: Rule,
    F: Fn() -> R,
{
    fn parse(&self, p: &mut Parser) -> bool {
        self().parse(p)
    }
}

impl Rule for &str {
    fn parse(&self, p: &mut Parser) -> bool {
        let matches = if p.case_sensitive {
            p.input[p.offset..].starts_with(self)
        } else {
            p.input[p.offset..(p.offset + self.len())].eq_ignore_ascii_case(self)
        };
        if matches {
            p.offset += self.len();
            true
        } else {
            false
        }
    }
}

impl Rule for RangeInclusive<char> {
    fn parse(&self, p: &mut Parser) -> bool {
        if let Some(c) = p.input[p.offset..].chars().next() {
            let matches = if p.case_sensitive {
                self.contains(&c)
            } else {
                self.contains(&c.to_ascii_uppercase()) || self.contains(&c.to_ascii_lowercase())
            };
            if matches {
                p.offset += c.len_utf8();
                return true;
            }
        }
        false
    }
}

macro_rules! impl_rule_for_tuple {
    ($($x:ident)+) => {
        impl<$($x),+> Rule for ($($x),+,) where $($x: Rule),+, {
            fn parse(&self, p: &mut Parser) -> bool {
                let save = p.save();
                let mut lambda = || {
                    #[allow(non_snake_case)]
                    let ($($x),+,) = self;
                    $(if !$x.parse(p) {return false; })+
                    true
                };
                if lambda() {
                    true
                } else {
                    p.restore(save);
                    false
                }
            }
        }
    };
}

#[derive(Clone)]
pub struct Alt<T>(pub T);

macro_rules! impl_rule_for_alt {
    ($($x:ident)+) => {
        impl<$($x),+> Rule for Alt<($($x),+,)> where $($x: Rule),+ {
            fn parse(&self, p: &mut Parser) -> bool {
                let save = p.save();
                #[allow(non_snake_case)]
                let ($($x),+,) = &self.0;
                $(
                    if $x.parse(p) {
                        return true
                    }
                    p.restore(save);
                )+
                false
            }
        }
    };
}

pub struct And<T>(pub T);

impl<T> Rule for And<T>
where
    T: Rule,
{
    fn parse(&self, p: &mut Parser) -> bool {
        let save = p.save();
        let result = self.0.parse(p);
        p.restore(save);
        result
    }
}

pub struct Dot;

impl Rule for Dot {
    fn parse(&self, p: &mut Parser) -> bool {
        if let Some(c) = p.input[p.offset..].chars().next() {
            p.offset += c.len_utf8();
            true
        } else {
            false
        }
    }
}

pub struct NoCase<T>(pub T);

impl<T> Rule for NoCase<T>
where
    T: Rule,
{
    fn parse(&self, p: &mut Parser) -> bool {
        let old_value = p.case_sensitive;
        p.case_sensitive = false;
        let result = self.0.parse(p);
        p.case_sensitive = old_value;
        result
    }
}

pub struct Not<T>(pub T);

impl<T> Rule for Not<T>
where
    T: Rule,
{
    fn parse(&self, p: &mut Parser) -> bool {
        let save = p.save();
        if self.0.parse(p) {
            p.restore(save);
            false
        } else {
            true
        }
    }
}

pub struct Opt<T>(pub T);

impl<T> Rule for Opt<T>
where
    T: Rule,
{
    fn parse(&self, p: &mut Parser) -> bool {
        let _ = self.0.parse(p);
        true
    }
}

pub struct Plus<T>(pub T);

impl<T> Rule for Plus<T>
where
    T: Rule,
{
    fn parse(&self, p: &mut Parser) -> bool {
        if !self.0.parse(p) {
            return false;
        }
        while self.0.parse(p) {}
        true
    }
}

#[macro_export]
macro_rules! plus {
    ($($item:expr),+ $(,)?) => {
        crate::preprocess::peg::Plus((
            $($item),+,
        ))
    };
}

pub struct Star<T>(pub T);

impl<T> Rule for Star<T>
where
    T: Rule,
{
    fn parse(&self, p: &mut Parser) -> bool {
        while self.0.parse(p) {}
        true
    }
}

#[macro_export]
macro_rules! star {
    ($($item:expr),+ $(,)?) => {
        crate::preprocess::peg::Star((
            $($item),+,
        ))
    };
}

pub struct Tag<T>(pub &'static str, pub T);

impl<T> Rule for Tag<T>
where
    T: Rule,
{
    fn parse(&self, p: &mut Parser) -> bool {
        let save = p.save();
        let index = p.captures.len();
        let start_offset = p.offset;
        p.captures.push(RawCapture {
            kind: self.0,
            span: 0..0,
            subtree_len: None,
        });
        if self.1.parse(p) {
            let subtree_len = NonZero::new(p.captures.len() - index).unwrap();
            p.captures[index].span = start_offset..p.offset;
            p.captures[index].subtree_len = Some(subtree_len);
            true
        } else {
            p.restore(save);
            false
        }
    }
}

pub struct EOF;

impl Rule for EOF {
    fn parse(&self, p: &mut Parser) -> bool {
        p.offset >= p.input.len()
    }
}

macro_rules! impl_rule_for_many {
    () => {};
    ($head:ident $($tail:ident)*) => {
        impl_rule_for_alt!($head $($tail)*);
        impl_rule_for_tuple!($head $($tail)*);
        impl_rule_for_many!($($tail)*);
    }
}

impl_rule_for_many!(A B C D E F G H I J);

#[cfg(test)]
mod test {
    use insta::assert_debug_snapshot;

    use super::*;

    fn parse<T: Rule>(rule: &T, input: &str) {
        let mut p = Parser::new(input);
        let rule = (Ref(rule), EOF);
        assert!(rule.parse(&mut p));
    }

    fn parse_err<T: Rule>(rule: &T, input: &str) {
        let mut p = Parser::new(input);
        let rule = (Ref(rule), EOF);
        assert!(!rule.parse(&mut p));
    }

    #[test]
    fn test_char_range() {
        let rule = plus!('0'..='9');
        parse(&rule, "0");
        parse(&rule, "123");
        parse(&rule, "9");
    }

    #[test]
    fn test_combinator_tuples() {
        let foo = ("f", "o", "o");
        let bar = "bar";
        let rule = (foo, " ", bar);
        parse(&rule, "foo bar");
    }

    #[test]
    fn test_combinator_fn_wrapper() {
        let foo = || "foo";
        let bar = (foo, " ", foo);
        parse(&bar, "foo foo");
    }

    #[test]
    fn test_combinator_alt() {
        let item = Alt(("foo", "bar", "baz"));
        let rule = (Ref(&item), ", ", Ref(&item));
        parse(&rule, "foo, bar");
        parse(&rule, "bar, baz");
        parse(&rule, "baz, foo");
    }

    #[test]
    fn test_combinator_and() {
        let has = |x| (star!(Not(x), Dot), x);
        let foo_bar = (And(has("foo")), And(has("bar")), star!(Dot));
        parse(&foo_bar, "foo bar");
        parse(&foo_bar, "bar foo");
        parse_err(&foo_bar, "foo foo");
        parse_err(&foo_bar, "bar bar");
    }

    #[test]
    fn test_combinator_dot() {
        let rule = ("foo", Dot, "bar");
        parse(&rule, "foo bar");
        parse(&rule, "foodbar");
        parse(&rule, "foo\nbar");
    }

    #[test]
    fn test_combinator_nocase() {
        let rule = NoCase(("foo-", 'a'..='z', 'A'..='Z'));
        parse(&rule, "foo-ab");
        parse(&rule, "fOO-cD");
        parse(&rule, "Foo-Ef");
        parse(&rule, "FOO-GH");
    }

    #[test]
    fn test_mixed_case() {
        let rule = ("foo ", NoCase("bar"), " baz");
        parse(&rule, "foo bar baz");
        parse(&rule, "foo BAR baz");
        parse_err(&rule, "FOO bar baz");
        parse_err(&rule, "foo bar BAZ");
    }

    #[test]
    fn test_combinator_not() {
        let rule = (Not("foo"), star!(Dot));
        parse(&rule, "bar");
        parse(&rule, "barfoo");
        parse_err(&rule, "foobar");
    }

    #[test]
    fn test_combinator_opt() {
        let rule = ("foo", Opt(" "), "bar");
        parse(&rule, "foobar");
        parse(&rule, "foo bar");
        parse_err(&rule, "foo  bar");
    }

    #[test]
    fn test_combinator_star() {
        let item = "foo";
        let csv = (item, star!(", ", item));
        parse(&csv, "foo");
        parse(&csv, "foo, foo");
        parse(&csv, "foo, foo, foo");
    }

    #[test]
    fn test_captures() {
        let num = Tag("num", plus!('0'..='9'));
        let rule = star!(Alt((num, Dot)));
        let mut p = Parser::new("I have 123 gems and 45 torches.");
        assert!(rule.parse(&mut p));
        let results: Vec<_> = p.iter().map(|x| (x.span(), x.text())).collect();
        assert_debug_snapshot!(results, @r#"
        [
            (
                7..10,
                "123",
            ),
            (
                20..22,
                "45",
            ),
        ]
        "#);
    }

    #[test]
    fn test_nested_captures() {
        let letter = || 'a'..='z';
        let user = Tag("user", plus!(letter));
        let domain = Tag("domain", (letter, star!(Opt("."), letter)));
        let email = Tag("email", (user, "@", domain));
        let rule = star!(Alt((email, Dot)));
        let mut p = Parser::new("Send to alice@foo.net or bob@bar.com.");
        assert!(rule.parse(&mut p));

        // Make sure the capture groups were correct
        let mut results = Vec::new();
        for email in p.iter() {
            assert_eq!(email.kind(), "email");
            for group in email.children() {
                results.push(format!("{}: {}", group.kind(), group.text()));
            }
        }
        assert_debug_snapshot!(results, @r#"
        [
            "user: alice",
            "domain: foo.net",
            "user: bob",
            "domain: bar.com",
        ]
        "#);

        // Make sure &str lifetimes outlive the Capture structs
        let slices: Vec<&str> = p
            .iter()
            .flat_map(|email| email.children())
            .map(|group| group.text())
            .collect();
        assert_debug_snapshot!(slices, @r#"
        [
            "alice",
            "foo.net",
            "bob",
            "bar.com",
        ]
        "#);
    }
}
