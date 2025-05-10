use crate::{
    preprocess::peg::{Alt, And, Dot, EOF, Not, Opt, Parser, Rule, Star, Tag},
    star,
    world::Stat,
};

pub fn print_labels(b: &Stat) {
    let mut parser = Parser::new(&b.code);
    if !parse_program().parse(&mut parser) {
        eprintln!("Couldn't parse stat's code: {:?}", b.code);
        return;
    }

    for group in parser.iter() {
        if group.event.kind == "label" {
            let span = group.event.span.clone();
            let s = &b.code[span];
            println!("- {}", s);
        }
    }
}

fn parse_program() -> impl Rule {
    (Opt((parse_line, star!(("\n", parse_line)))), EOF)
}

fn parse_line() -> impl Rule {
    Alt((parse_label_line, parse_any_line))
}

fn parse_any_line() -> impl Rule {
    (star!(Not("\n"), Dot), eol)
}

fn parse_label_line() -> impl Rule {
    let allowed = Alt(('A'..='Z', 'a'..='z', '0'..='9', "_", "@", "~"));
    (":", Tag("label", star!(allowed)), eol)
}

fn eol() -> impl Rule {
    Alt((And("\n"), EOF))
}
