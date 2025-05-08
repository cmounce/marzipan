use std::ops::RangeInclusive;

use anyhow::{Result, anyhow, bail};

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
    fn parse(&self, p: &mut Parser) -> Result<()>;
}

struct Ref<T>(T);

impl<T> Rule for Ref<&T>
where
    T: Rule,
{
    fn parse(&self, p: &mut Parser) -> Result<()> {
        self.0.parse(p)
    }
}

impl<R, F> Rule for F
where
    R: Rule,
    F: Fn() -> R,
{
    fn parse(&self, p: &mut Parser) -> Result<()> {
        self().parse(p)
    }
}

impl Rule for &str {
    fn parse(&self, p: &mut Parser) -> Result<()> {
        if p.input[p.offset..].starts_with(self) {
            p.offset += self.len();
            Ok(())
        } else {
            bail!("No match")
        }
    }
}

impl Rule for RangeInclusive<char> {
    fn parse(&self, p: &mut Parser) -> Result<()> {
        if let Some(c) = p.input[p.offset..].chars().next() {
            if self.contains(&c) {
                p.offset += c.len_utf8();
                return Ok(());
            }
        }
        bail!("No match")
    }
}

macro_rules! impl_rule_for_tuple {
    ($($x:ident)+) => {
        impl<$($x),+> Rule for ($($x),+,) where $($x: Rule),+, {
            fn parse(&self, p: &mut Parser) -> Result<()> {
                let save = p.save();
                let mut lambda = || {
                    #[allow(non_snake_case)]
                    let ($($x),+,) = self;
                    $($x.parse(p)?;)+
                    Ok(())
                };
                match lambda() {
                    Ok(x) => Ok(x),
                    Err(e) => {
                        p.restore(save);
                        Err(e)
                    }
                }
            }
        }
    };
}

struct Alt<T>(T);

macro_rules! impl_rule_for_alt {
    ($($x:ident)+) => {
        impl<$($x),+> Rule for Alt<($($x),+,)> where $($x: Rule),+ {
            fn parse(&self, p: &mut Parser) -> Result<()> {
                let save = p.save();
                #[allow(non_snake_case)]
                let ($($x),+,) = &self.0;
                $(
                    if $x.parse(p).is_ok() {
                        return Ok(())
                    }
                    p.restore(save);
                )+
                bail!("No match")
            }
        }
    };
}

pub struct Star<T>(pub T);

impl<T> Rule for Star<T>
where
    T: Rule,
{
    fn parse(&self, p: &mut Parser) -> Result<()> {
        while self.0.parse(p).is_ok() {}
        Ok(())
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
    fn parse(&self, p: &mut Parser) -> Result<()> {
        if p.offset < p.input.len() {
            bail!("No match");
        }
        Ok(())
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

    fn parse<T: Rule>(rule: &T, input: &str) -> Result<()> {
        let mut p = Parser::new(input);
        let rule = (Ref(rule), EOF);
        rule.parse(&mut p)
    }

    #[test]
    fn test_char_range() -> Result<()> {
        let num = '0'..='9';
        let rule = (Ref(&num), star!(Ref(&num)));
        parse(&rule, "0")?;
        parse(&rule, "123")?;
        parse(&rule, "9")?;
        Ok(())
    }

    #[test]
    fn test_combinator_tuples() {
        let foo = ("f", "o", "o");
        let bar = "bar";
        let rule = (foo, " ", bar);
        let mut p = Parser::new("foo bar");
        rule.parse(&mut p).unwrap();
    }

    #[test]
    fn test_combinator_fn_wrapper() {
        let foo = || "foo";
        let bar = (foo, " ", foo);
        let mut p = Parser::new("foo foo");
        bar.parse(&mut p).unwrap();
    }

    #[test]
    fn test_combinator_alt() -> Result<()> {
        let item = Alt(("foo", "bar", "baz"));
        let rule = (Ref(&item), ", ", Ref(&item));
        parse(&rule, "foo, bar")?;
        parse(&rule, "bar, baz")?;
        parse(&rule, "baz, foo")?;
        Ok(())
    }

    #[test]
    fn test_combinator_star() -> Result<()> {
        let item = "foo";
        let csv = (item, star!(", ", item));
        parse(&csv, "foo")?;
        parse(&csv, "foo, foo")?;
        parse(&csv, "foo, foo, foo")?;
        Ok(())
    }
}
