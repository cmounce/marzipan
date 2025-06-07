use crate::{
    peg::ParseState,
    plus,
    preprocess::peg::{Alt, And, Dot, EOF, NoCase, Not, Opt, Parser, Rule, Tag},
    star,
    world::Stat,
};

pub fn print_labels(b: &Stat) {
    let code = &b.code;
    let mut ps = ParseState::new(code);
    assert!(grammar::program(&mut ps), "New parser couldn't parse: {:?}", code);

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

mod grammar {
    use peg_macro::grammar;

    grammar! {
        program = (line ("\n" line)*)? EOI;
        line = label_line / statement;
        statement = movement+ command? / command / text;
        movement = ("/" / "?") s direction;
        text = !("#" / "/" / "?") (!"\n" ANY)*;

        label_line = ":" label eol;

        command = "#" bare_command;
        bare_command = bare_compound_command / bare_simple_command;
        @icase
        bare_compound_command = (
            ("give" / "take") sp counter sp value /
            "if" sp condition /
            "try" sp direction
        ) s (statement / bare_command);
        @icase
        bare_simple_command = (
            &'b'..'c' (
                "become" sp kind /
                "bind" sp word /
                "change" sp kind sp kind /
                "char" sp value /
                "clear" sp word /
                "cycle" sp value
            ) /
            &'d'..'l' (
                "die" eow /
                "end" "game"? eow /
                "go" sp direction /
                "idle" eow /
                "lock" eow
            ) /
            &'p'..'s' (
                "play" eow (!"\n" ANY)* /
                "put" sp direction sp kind /
                "restart" eow /
                "restore" sp message /
                "send" sp message /
                "set" sp word /
                "shoot" sp direction
            ) /
            &'t'..'z' (
                "throwstar" sp direction /
                "unlock" eow /
                "walk" sp direction /
                "zap" sp message
            ) /
            message // shorthand send
        ) s eol; // note: this allows trailing (ignored) whitespace

        //
        //  Common definitions
        //

        // Color names
        @icase
        color = ("blue" / "green" / "cyan" / "red" / "purple" / "yellow" / "white") eow;

        // Conditions
        condition = ("not"i sp)* base_condition;
        @icase
        base_condition =
            // These need `eow`/`sp` immediately after each literal because each one
            // could potentially appear in a flag name as a prefix: `#set allignedxyz`
            "alligned" eow /
            "any" sp kind /
            "blocked" sp direction /
            "contact" eow /
            "energized" eow /
            word; // flag name

        // Counter names
        counter = ("ammo" / "gems" / "health" / "score" / "time" / "torches") eow;

        // Directions
        direction = (direction_modifier sp)* base_direction;
        @icase
        direction_modifier = ("cw" / "ccw" / "opp" / "rndp") eow;
        @icase
        base_direction = (
            "flow" / "rnd" ("ne" / "ns")? / "seek" /        // dynamic
            "north" / "south" / "east" / "west" / "idle" /  // long forms
            "n" / "s" / "e" / "w" / "i"                     // short forms
        ) eow;

        // Labels (defined locations in the code)
        // Examples: foo, namespace~foo, foo.local, .local, @
        label = #Label:(namespace? (label_name / #Anon:"@"));
        namespace = #Namespace:label_word "~";
        label_name = label_local / label_global label_local?;
        label_global = #Global:label_word;
        label_local = "." #Local:label_word;
        label_word = word_char+; // labels can start with 0-9

        // Messages (references to labels)
        // Examples: foo, all:namespace~bar.baz, @b, @f
        message = #Message:(recipient? message_name);
        recipient = #Recipient:word ":";
        message_name = namespace? (label_name / #Anon:anon_message);
        anon_message = "@" ("b" / "f");

        // Tile kinds
        kind = (color sp)? base_kind;
        @icase
        base_kind = (
            &'a'..'b' ("ammo" / "bear" / "blinkwall" / "bomb" / "boulder" / "breakable" / "bullet") /
            &'c'..'e' ("clockwise" / "counter" / "door" / "duplicator" / "empty" / "energizer") /
            &'f'..'k' ("fake" / "forest" / "gem" / "head" / "invisible" / "key") /
            &'l'..'o' ("line" / "lion" / "monitor" / "normal" / "object") /
            &'p'..'r' ("passage" / "player" / "pusher" / "ricochet" / "ruffian") /
            &"s" ("scroll" / "segment" / "shark" / "slider"("ew"/"ns") / "slime" / "solid" / "spinninggun" / "star") /
            &'t'..'w' ("tiger" / "torch" / "transporter" / "water")
        ) eow;

        //
        // Generic helpers
        //

        eol = &("\n" / EOI);
        eow = !('a'..'z'i / '0'..'9' / "_");
        s = " "*;
        sp = " "+;
        value = '0'..'9'+;
        word = !'0'..'9' word_char+;
        word_char = ('a'..'z'i / '0'..'9' / "_");
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
        Alt((shorthand_send, Box::new(line) as Box<dyn Rule>)),
    )
}

/// `send` without a preceding `#`
fn bare_send() -> impl Rule {
    (NoCase("send"), ww, label_reference, w, eol)
}

/// `send` without a send keyword
fn shorthand_send() -> impl Rule {
    let af = (
        And('a'..='f'),
        Alt((
            "become", "bind", "change", "char", "clear", "cycle", "die", "end", "endgame",
        )),
    );
    let gr = (
        And('g'..='r'),
        Alt((
            "go", "idle", "if", "lock", "play", "put", "restart", "restore",
        )),
    );
    let sz = (
        And('s'..='z'),
        Alt((
            "send",
            "set",
            "shoot",
            "take",
            "throwstar",
            "try",
            "unlock",
            "walk",
            "zap",
        )),
    );
    let command = Alt((af, gr, sz));
    (Not(command), label_reference, w, eol)
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
        ('a'..='z', star!(word_char)),
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

fn label_reference() -> impl Rule {
    Tag(
        "ref",
        (
            Opt((Tag("dest", plus!(word_char)), ":")),
            Tag("name", label_name),
        ),
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
                "ref" => format!("[{}]", group.text()),
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
