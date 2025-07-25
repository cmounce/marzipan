use std::ops::Range;

use compact_str::CompactString;
use grammar::Tag;

use crate::{error::Context, peg::ParseState, world::Stat};

pub type ParsedStat = Vec<Chunk>;

#[derive(Debug)]
pub enum Chunk {
    Verbatim(String),
    Label {
        is_ref: bool,
        is_anon: bool,
        name: LabelName,
    },
}

#[derive(Clone, Debug, Default)]
pub struct LabelName {
    pub namespace: Option<CompactString>,
    pub name: CompactString,
    pub local: Option<CompactString>,
    pub span: Range<usize>,
}

pub fn parse_stat_labels(stat: &Stat, ctx: &Context) -> ParsedStat {
    let code = &stat.code;
    let mut parser = ParseState::new(code);
    assert!(
        grammar::program(&mut parser),
        "Couldn't parse code: {:?}",
        code
    );

    for cap in parser.walk_captures() {
        match cap.kind() {
            Tag::WarnTrailing => {
                ctx.with_span(cap.span())
                    .warning("trailing characters at end of line");
            }
            _ => {}
        }
    }

    // Find all #Label captures and record which ones were references
    let mut label_captures = vec![];
    for cap in parser.captures() {
        match cap.kind() {
            Tag::Label => label_captures.push((Tag::Label, cap)),
            Tag::Reference => {
                let label = cap.children().find(|c| c.kind() == Tag::Label).unwrap();
                label_captures.push((Tag::Reference, label));

                // Detect invalid recipients.
                // This should probably happen later in processing, but
                // we'd need an AST that can track spans for message recipients.
                let mut recipient = None;
                let (mut anon, mut local) = (false, false);
                for child in cap.walk_children() {
                    match child.kind() {
                        Tag::Anon => anon = true,
                        Tag::Local => local = true,
                        Tag::Recipient => recipient = Some(child),
                        _ => {}
                    }
                }
                if let Some(recipient) = recipient {
                    let ctx = ctx.with_span(recipient.span());
                    if anon {
                        ctx.error("message targets not allowed for anonymous labels");
                    } else if local {
                        ctx.error("message targets not supported for local labels");
                    }
                }
            }
            _ => {}
        }
    }

    // Convert #Labels into (span, chunk) pairs
    let span_chunks = label_captures.iter().map(|(tag, cap)| {
        let mut name = LabelName::default();
        name.span = cap.span();
        let mut is_anon = false;
        for child in cap.children() {
            match child.kind() {
                Tag::Namespace => name.namespace = Some(child.text().into()),
                Tag::Anon | Tag::Global => {
                    name.name = child.text().into();
                    is_anon = child.kind() == Tag::Anon;
                }
                Tag::Local => name.local = Some(child.text().into()),
                _ => unreachable!(),
            }
        }
        let chunk = Chunk::Label {
            is_ref: *tag == Tag::Reference,
            is_anon,
            name,
        };
        (cap.span(), chunk)
    });

    // Split code along #Label boundaries
    let mut last_index = 0;
    let mut result = vec![];
    for (span, chunk) in span_chunks {
        let Range { start, end } = span;
        if last_index < span.start {
            result.push(Chunk::Verbatim(code[last_index..start].into()));
        }
        result.push(chunk);
        last_index = end;
    }
    if last_index < code.len() {
        result.push(Chunk::Verbatim(code[last_index..code.len()].into()));
    }
    result
}

mod grammar {
    use mzp_peg_macro::grammar;

    grammar! {
        program = (line ("\n" line)*)? EOI;
        line = label_line / statement / text;
        statement = movement+ command? / command;
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
        ) s (statement / bare_command / eol);
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
        ) warn_trailing eol;

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

        // Labels
        // Examples: foo, namespace~foo, foo.local, .local, @
        label = #Label:(namespace? (label_name / #Anon:"@"));
        namespace = #Namespace:label_word "~";
        label_name = label_local / label_global label_local?;
        label_global = #Global:label_word;
        label_local = "." #Local:label_word;
        label_word = word_char+; // labels can start with 0-9

        // References to labels
        // Examples: foo, all:namespace~bar.baz, @b, @f
        message = #Reference:(recipient? #Label:message_name);
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

        // Warnings
        warn_trailing = (#WarnTrailing:(!eol ANY)+)?; // TODO: Document precedence rules

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

#[cfg(test)]
mod test {
    use std::fs;

    use insta::assert_snapshot;

    use crate::peg::ParseState;

    use super::{grammar::Tag, *};

    fn parse<T: Clone, F: Fn(&mut ParseState<T>) -> bool>(rule: F, input: &str) {
        use crate::peg::backend::LowLevel;
        let mut p = ParseState::new(input);
        assert!(rule(&mut p));
        assert!(p.eoi());
    }

    fn parse_err<T: Clone, F: Fn(&mut ParseState<T>) -> bool>(rule: F, input: &str) {
        use crate::peg::backend::LowLevel;
        let mut p = ParseState::new(input);
        assert!(!rule(&mut p) || !p.eoi());
    }

    #[test]
    fn test_label() {
        parse(grammar::label, "foo");
        parse(grammar::label, ".loop");
        parse(grammar::label, "foo.loop");
        parse(grammar::label, "ns~foo");
        parse(grammar::label, "ns~.loop");

        parse_err(grammar::label, "foo.");
        parse_err(grammar::label, "foo.bar.baz");
        parse_err(grammar::label, "foo~bar~baz");
        parse_err(grammar::label, "~foo");
    }

    #[test]
    fn test_direction() {
        parse(grammar::direction, "n");
        parse(grammar::direction, "north");
        parse(grammar::direction, "rndp rndne");
        parse(grammar::direction, "opp   seek");
        parse(grammar::direction, "cw cw cw flow");
    }

    #[test]
    fn test_condition() {
        parse(grammar::condition, "alligned");
        parse(grammar::condition, "blocked seek");
        parse(grammar::condition, "not blocked rndp seek");
        parse(grammar::condition, "any red lion");
        parse(grammar::condition, "any bear");
    }

    #[test]
    fn test_label_detection() {
        let input = fs::read_to_string("tests/labels/find-all.txt").unwrap();
        let mut parser = ParseState::new(&input);
        assert!(grammar::program(&mut parser));

        let mut result = String::new();
        let mut last_index = 0;
        for group in parser.captures() {
            let before = &input[last_index..group.span().start];
            let inner = match group.kind() {
                Tag::Label => format!("({})", group.text()),
                Tag::Reference => format!("[{}]", group.text()),
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
