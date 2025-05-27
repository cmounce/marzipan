use std::ops::RangeInclusive;

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

    pub fn range(&mut self, r: RangeInclusive<char>) -> bool {
        if let Some(next) = self.input[self.offset..].chars().next() {
            if r.contains(&next) {
                self.offset += next.len_utf8();
                return true;
            }
        }
        false
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

        word_head = "_" / 'A'..'Z' / 'a'..'z';
        word_tail = word_head / '0'..'9';
        plain_word = word_head word_tail*;
        word = "(" plain_word ")";

        words = "(" (plain_word ("," plain_word)*)? ")";
        nested_choice = "(" ("a" / "b") ("c" / "d") ")";
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
}
