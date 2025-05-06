use crate::{
    preprocess::peg::{EOF, Rule, Star},
    star,
};

fn parse_program() -> impl Rule {
    (star!(parse_line), EOF)
}

fn parse_line() -> impl Rule {
    (star!("foo"), "\n")
}
