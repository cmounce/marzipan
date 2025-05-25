use peg_macro::grammar;

#[derive(Debug)]
#[allow(dead_code)]
pub enum Foo {
    Bar(usize),
    Baz(String),
}

pub struct ParseState {
    pub input: String,
    pub offset: usize,
}

pub struct Savepoint {
    offset: usize,
}

impl ParseState {
    pub fn new(s: &str) -> Self {
        Self {
            input: s.into(),
            offset: 0,
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
}

fn parse_foo(p: &mut ParseState) -> bool {
    let save = p.save();
    if !parse_f(p) {
        return false;
    }
    if !parse_oo(p) {
        p.restore(save);
        return false;
    }
    true
}

fn parse_f(p: &mut ParseState) -> bool {
    p.literal("f")
}

fn parse_oo(p: &mut ParseState) -> bool {
    p.literal("oo")
}

grammar! {
    generated_this;
    generated_that;
}

#[cfg(test)]
mod tests {
    use insta::assert_snapshot;

    use super::*;

    #[test]
    fn test_parser() {
        let mut p = ParseState::new("foo");
        assert!(parse_foo(&mut p));
        let mut p = ParseState::new("fee");
        assert!(!parse_foo(&mut p));
    }

    #[test]
    fn test_generated() {
        assert_snapshot!(generated_this(), @"I'm a generated function!");
        assert_snapshot!(generated_that(), @"I'm a generated function!");
    }
}
