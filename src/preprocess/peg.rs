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

    pub fn eof(&mut self) -> Result<()> {
        if self.input.len() == self.offset {
            Ok(())
        } else {
            bail!("No match")
        }
    }

    pub fn literal(&mut self, str: &str) -> Result<()> {
        if self.input[self.offset..].starts_with(str) {
            self.offset += str.len();
            Ok(())
        } else {
            bail!("No match")
        }
    }

    pub fn char<F>(&mut self, f: F) -> Result<()>
    where
        F: Fn(char) -> bool,
    {
        if let Some(c) = self.input[self.offset..].chars().next() {
            if f(c) {
                self.offset += c.len_utf8();
                return Ok(());
            }
        }
        bail!("No match")
    }

    pub fn star<F>(&mut self, mut f: F) -> Result<()>
    where
        F: FnMut(&mut Self) -> Result<()>,
    {
        let mut sp = self.save();
        while let Ok(_) = f(self) {
            sp = self.save();
        }
        self.restore(sp);
        Ok(())
    }
}

mod test {
    use super::*;

    #[test]
    fn test_hello() {
        let mut p = Parser::new("foo");
        p.literal("f").unwrap();
        p.star(|p| p.literal("o")).unwrap();
    }
}
