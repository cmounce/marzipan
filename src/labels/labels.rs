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
    (motion_prefix, Alt((command_line, label_line, any_line)))
}

fn motion_prefix() -> impl Rule {
    star!(Alt(("/", "?")), w, direction)
}

fn command_line() -> impl Rule {
    ("#", w, bare_command)
}

fn bare_command() -> impl Rule {
    Alt((bare_if, bare_send))
}

fn bare_if() -> impl Rule {
    (
        "if",
        w,
        condition,
        w,
        Opt(("then", w)),
        Box::new(line) as Box<dyn Rule>,
    )
}

fn bare_send() -> impl Rule {
    (NoCase("send"), ww, Tag("ref", label_name), w, eol)
}

fn any_line() -> impl Rule {
    (star!(Not("\n"), Dot), eol)
}

fn label_line() -> impl Rule {
    (":", Tag("label", label_name), eol)
}

fn tile_kind() -> impl Rule {
    (Opt((tile_color, ww)), tile_base_kind)
}

fn tile_color() -> impl Rule {
    (
        Alt(("blue", "green", "cyan", "red", "purple", "yellow", "white")),
        Not('a'..='z'),
    )
}

fn tile_base_kind() -> impl Rule {
    let ac = (
        And('a'..='c'),
        Alt((
            "ammo",
            "bear",
            "blinkwall",
            "bomb",
            "boulder",
            "breakable",
            "bullet",
            "clockwise",
            "counter",
        )),
    );
    let dk = (
        And('d'..='k'),
        Alt((
            "door",
            "duplicator",
            "empty",
            "energizer",
            "fake",
            "forest",
            "gem",
            "head",
            "invisible",
            "key",
        )),
    );
    let lr = (
        And('l'..='r'),
        Alt((
            "line", "lion", "monitor", "normal", "object", "passage", "player", "pusher",
            "ricochet", "ruffian",
        )),
    );
    let s = (
        And("s"),
        Alt((
            "scroll",
            "segment",
            "shark",
            "sliderew",
            "sliderns",
            "slime",
            "solid",
            "spinninggun",
            "star",
        )),
    );
    let tz = (
        And('t'..='z'),
        Alt(("tiger", "torch", "transporter", "water")),
    );
    Alt((ac, dk, lr, s, tz))
}

fn condition() -> impl Rule {
    let base = Alt((
        "alligned",
        ("any", ww, tile_kind),
        ("blocked", ww, direction),
        "contact",
        "energized",
    ));
    (star!("not", ww), base)
}

fn direction() -> impl Rule {
    let modifier = Alt(("cw", "ccw", "rndp", "opp"));
    (star!(modifier, ww), base_direction, Not('a'..='z'))
}

fn base_direction() -> impl Rule {
    // Some directions are prefixes of others, which can break parsing.
    // To prevent this, we roughly order the strings from longest to shortest.
    let dynamic = Alt(("flow", "rndne", "rndns", "rnd", "seek"));
    let long = Alt(("north", "south", "east", "west", "idle"));
    let short = Alt(("n", "s", "e", "w", "i"));
    (Alt((dynamic, long, short)), Not('a'..='z'))
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
    use std::fs;

    use insta::{assert_debug_snapshot, assert_snapshot};

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
    fn test_direction() {
        parse(&direction, "n");
        parse(&direction, "north");
        parse(&direction, "rndp rndne");
        parse(&direction, "opp   seek");
        parse(&direction, "cw cw cw flow");
    }

    #[test]
    fn test_condition() {
        parse(&condition, "alligned");
        parse(&condition, "blocked seek");
        parse(&condition, "not blocked rndp seek");
        parse(&condition, "any red lion");
        parse(&condition, "any bear");
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

    #[test]
    fn test_label_detection() {
        let input = fs::read_to_string("tests/labels/find-all.txt").unwrap();
        let mut parser = Parser::new(&input);
        assert!(program.parse(&mut parser));

        let mut result = String::new();
        let mut last_index = 0;
        for group in parser.iter() {
            let before = &input[last_index..group.span().start];
            let inner = match group.kind() {
                "label" => format!("({})", group.text()),
                _ => group.text().into(),
            };
            result.push_str(before);
            result.push_str(&inner);
            last_index = group.span().end;
        }
        result.push_str(&input[last_index..]);
        assert_snapshot!(result);
    }
}
