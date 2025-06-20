use compact_str::CompactString;

use crate::world::Board;

use super::{
    parse::{Chunk, ParsedStat, parse_stat_labels},
    sanitize::Registry,
};

pub fn process_labels(board: &mut Board) {
    // Parse stats into chunks
    let mut stats: Vec<ParsedStat> = board
        .stats
        .iter()
        .map(|stat| parse_stat_labels(&stat))
        .collect();
    let mut registry = Registry::new();

    resolve_local_labels(&mut stats);

    // Replace each label with its sanitized equivalent
    for stat in stats.iter_mut() {
        for chunk in stat.iter_mut() {
            match chunk {
                Chunk::Verbatim(_) => {}
                Chunk::Label {
                    name,
                    is_ref: _,
                    is_anon: _,
                } => {
                    name.name = registry.sanitize(name).into();
                }
            }
        }
    }

    // Assign names to anonymous labels
    anonymous_forward_pass(&mut stats, &mut registry);
    anonymous_backward_pass(&mut stats);

    // Join chunks together and replace old stats' code
    for (old_stat, parsed_stat) in board.stats.iter_mut().zip(stats.into_iter()) {
        let new_code = parsed_stat
            .into_iter()
            .map(|chunk| match chunk {
                Chunk::Verbatim(s) => s,
                Chunk::Label {
                    is_ref: _,
                    is_anon: _,
                    name,
                } => name.name.into(),
            })
            .collect();
        old_stat.code = new_code;
    }
}

/// Resolve ".local" labels to "name.local" form.
fn resolve_local_labels(stats: &mut [ParsedStat]) {
    for stat in stats.iter_mut() {
        let mut section = CompactString::const_new("");
        for chunk in stat.iter_mut() {
            match chunk {
                Chunk::Label {
                    is_ref,
                    is_anon: false,
                    name: label,
                } => {
                    // Handle cases where either one of (section name, label) is missing.
                    // If both are present ("name.local") then the label is already fully resolved
                    // and there is nothing to do.
                    if label.name.is_empty() {
                        // Expand :.local to :name.local
                        assert!(label.local.is_some());
                        label.name = section.clone()
                    } else if label.local.is_none() {
                        // Interpret label :name as start of new section.
                        // Only label definitions do this; label references have no effect.
                        if !*is_ref {
                            section = label.name.clone();
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

/// Simultaneously:
/// 1. Assign names to anonymous labels.
/// 2. Resolve anonymous backward references to their label names.
fn anonymous_forward_pass(stats: &mut [ParsedStat], registry: &mut Registry) {
    let mut label_names = vec![];
    for stat in stats.iter_mut() {
        let mut i = 0;
        for chunk in stat.iter_mut() {
            match chunk {
                Chunk::Label {
                    is_ref: false,
                    is_anon: true,
                    name,
                } => {
                    if i == label_names.len() {
                        label_names.push(registry.gen_anonymous());
                    }
                    name.name = label_names[i].clone();
                    i += 1;
                }
                Chunk::Label {
                    is_ref: true,
                    is_anon: true,
                    name,
                } => {
                    if name.name == "@b" {
                        // TODO: a more explicit check that a "before" label exists
                        name.name = label_names[i - 1].clone();
                    }
                }
                _ => {}
            }
        }
    }
}

/// Resolve anonymous forward references to their label names.
fn anonymous_backward_pass(stats: &mut [ParsedStat]) {
    for stat in stats.iter_mut() {
        let mut last_name = None;
        for chunk in stat.iter_mut().rev() {
            match chunk {
                Chunk::Label {
                    is_ref: false,
                    is_anon: true,
                    name,
                } => {
                    last_name = Some(name.name.clone());
                }
                Chunk::Label {
                    is_ref: true,
                    is_anon: true,
                    name,
                } => {
                    if name.name == "@f" {
                        name.name = last_name.clone().unwrap();
                    }
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod test {
    use std::fs;

    use insta::assert_snapshot;

    use crate::world::{Board, Stat};

    use super::process_labels;

    fn board_from_text(path: &str) -> Board {
        let input = fs::read_to_string(path).unwrap();
        let codes: Vec<String> = input.split("---\n").map(|s| s.into()).collect();
        let blank = fs::read("tests/blank.brd").unwrap();
        let mut board = Board::from_bytes(&blank).unwrap();
        board.stats = codes
            .into_iter()
            .map(|code| Stat {
                x: 1,
                y: 1,
                x_step: 0,
                y_step: 0,
                cycle: 3,
                p1: 2,
                p2: 0,
                p3: 0,
                follower: -1,
                leader: -1,
                under_element: 0,
                under_color: 0,
                instruction_pointer: 0,
                bind_index: 0,
                code,
            })
            .collect();
        board
    }

    fn board_to_text(board: Board) -> String {
        let codes: Vec<_> = board.stats.into_iter().map(|stat| stat.code).collect();
        codes.join("---\n")
    }

    #[test]
    fn test_label_sanitization() {
        let mut board = board_from_text("tests/labels/sanitize.txt");
        process_labels(&mut board);
        assert_snapshot!(board_to_text(board));
    }

    #[test]
    fn test_anonymous_labels() {
        let mut board = board_from_text("tests/labels/anonymous.txt");
        process_labels(&mut board);
        assert_snapshot!(board_to_text(board));
    }

    #[test]
    fn test_local_labels() {
        let mut board = board_from_text("tests/labels/local.txt");
        process_labels(&mut board);
        assert_snapshot!(board_to_text(board));
    }
}
