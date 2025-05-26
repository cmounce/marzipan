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

#[cfg(test)]
mod tests {
    use peg_macro::grammar;

    use super::*;

    grammar! {
        fake_csv = line "\n" line;
        line = item "," item;
        item = "foo" / "bar";
    }

    #[test]
    fn test_parser() {
        let mut p = ParseState::new("foo");
        assert!(parse_foo(&mut p));
        let mut p = ParseState::new("fee");
        assert!(!parse_foo(&mut p));
    }

    #[test]
    fn test_generated() {
        let mut p = ParseState::new("foo,bar\nbar,foo");
        assert!(fake_csv(&mut p));
        let mut p = ParseState::new("foo,foo\nfoo;foo");
        assert!(!fake_csv(&mut p));
    }
}
