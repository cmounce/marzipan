use std::ops::RangeInclusive;

// TODO: Add offsets
pub enum Tag {
    Open(&'static str),
    Close,
}
pub struct Parser {
    input: String,
    offset: usize,
    output: Vec<Tag>,
}

#[derive(Clone, Copy)]
pub struct Savepoint {
    offset: usize,
    num_tags: usize,
}

impl Parser {
    pub fn new(input: &str) -> Self {
        Self {
            input: input.into(),
            offset: 0,
            output: Vec::new(),
        }
    }

    pub fn save(&self) -> Savepoint {
        Savepoint {
            offset: self.offset,
            num_tags: self.output.len(),
        }
    }

    pub fn restore(&mut self, sp: Savepoint) {
        self.offset = sp.offset;
        self.output.truncate(sp.num_tags);
    }
}

pub trait Rule {
    fn parse(&self, p: &mut Parser) -> bool;
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
        if p.input[p.offset..].starts_with(self) {
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
            if self.contains(&c) {
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
        Star((
            $($item),+,
        ))
    };
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

mod test {
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
        let num = '0'..='9';
        let rule = (Ref(&num), star!(Ref(&num)));
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
}
