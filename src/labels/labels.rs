use crate::{
    preprocess::peg::{Alt, And, Dot, EOF, Not, Opt, Rule, Star},
    star,
};

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
    (":", star!(allowed), eol)
}

fn eol() -> impl Rule {
    Alt((And("\n"), EOF))
}
