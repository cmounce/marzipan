use crate::{
    plus,
    preprocess::peg::{Alt, And, Dot, EOF, NoCase, Not, Opt, Parser, Rule, Tag},
    star,
    world::Stat,
};

pub fn print_labels(b: &Stat) {
    let mut parser = Parser::new(&b.code);
    if !program.parse(&mut parser) {
        eprintln!("Couldn't parse stat's code: {:?}", b.code);
        return;
    }

    for capture in parser.iter() {
        if capture.kind() == "label" {
            println!("- {}", capture.text());
        }
    }
}

fn program() -> impl Rule {
    (Opt((line, star!(("\n", line)))), EOF)
}

fn line() -> impl Rule {
    Alt((send, label_line, any_line))
}

fn send() -> impl Rule {
    ("#", w, NoCase("send"), ww, Tag("ref", label_name), w, eol)
}

fn any_line() -> impl Rule {
    (star!(Not("\n"), Dot), eol)
}

fn label_line() -> impl Rule {
    (":", Tag("label", label_name), eol)
}

fn label_name() -> impl Rule {
    let namespace = (plus!(word_char), "~");
    (
        Opt(namespace),
        star!(word_char),
        Opt((".", plus!(word_char))),
    )
}

fn word_char() -> impl Rule {
    Alt(('A'..='Z', 'a'..='z', '0'..='9', "_"))
}

fn eol() -> impl Rule {
    Alt((And("\n"), EOF))
}

fn w() -> impl Rule {
    star!(" ")
}

fn ww() -> impl Rule {
    plus!(" ")
}

#[cfg(test)]
mod test {
    use insta::assert_debug_snapshot;

    use crate::preprocess::peg::Ref;

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
    fn test_label_name() {
        parse(&label_name, "foo");
        parse(&label_name, ".loop");
        parse(&label_name, "foo.loop");
        parse(&label_name, "ns~foo");
        parse(&label_name, "ns~.loop");

        parse_err(&label_name, "foo.");
        parse_err(&label_name, "foo.bar.baz");
        parse_err(&label_name, "foo~bar~baz");
        parse_err(&label_name, "~foo");
    }

    #[test]
    fn test_references() {
        let mut p = Parser::new("#send foo");
        assert!(program.parse(&mut p));
        let result: Vec<_> = p
            .iter()
            .filter(|x| x.kind() == "ref")
            .map(|x| x.text())
            .collect();
        assert_debug_snapshot!(result, @r#"
        [
            "foo",
        ]
        "#);
    }
}
