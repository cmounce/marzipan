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

#[cfg(test)]
mod tests {
    use peg_macro::grammar;

    use super::*;

    grammar! {
        fake_csv = line "\n" line;
        line = item "," item;
        item = "foo" / "bar";

        option = "(" "x"? ")";
        plus = "(" "x"+ ")";
        star = "(" "x"* ")";
    }

    fn parse<T: Fn(&mut ParseState) -> bool>(rule: T, s: &str) -> bool {
        let mut p = ParseState::new(s);
        rule(&mut p)
    }

    #[test]
    fn test_generated() {
        let mut p = ParseState::new("foo,bar\nbar,foo");
        assert!(fake_csv(&mut p));
        let mut p = ParseState::new("foo,foo\nfoo;foo");
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
}
