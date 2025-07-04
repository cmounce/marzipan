use compact_str::CompactString;
use rustc_hash::FxHashMap;

use crate::{error::Context, world::Board};

use super::{
    parse::{Chunk, ParsedStat, parse_stat_labels},
    sanitize::Registry,
};

pub fn process_labels(board: &mut Board, ctx: &Context) {
    // Parse stats into chunks
    let mut stats: Vec<ParsedStat> = board
        .stats
        .iter()
        .enumerate()
        .map(|(index, stat)| parse_stat_labels(&stat, &ctx.with_stat(index)))
        .collect();
    let mut registry = Registry::new();

    resolve_local_labels(&mut stats);
    assign_named_labels(&mut stats, &mut registry);

    // Assign names to anonymous labels
    anonymous_forward_pass(&mut stats, &mut registry, ctx);
    anonymous_backward_pass(&mut stats, ctx);

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
        // Helper: Generate unique section strings like "touch$0"
        let mut i = 0;
        let mut make_section_id = |label_name: &str| -> CompactString {
            let result = format!("{}${}", &label_name, i);
            i += 1;
            result.into()
        };

        let mut namespace_to_section: FxHashMap<Option<CompactString>, CompactString> =
            FxHashMap::default();

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
                        label.name = if let Some(section) =
                            namespace_to_section.get(&label.namespace)
                        {
                            section.clone()
                        } else {
                            let section = make_section_id("");
                            namespace_to_section.insert(label.namespace.clone(), section.clone());
                            section
                        }
                    } else if label.local.is_none() {
                        // Interpret label :name as start of new section.
                        // Only label definitions do this; label references have no effect.
                        if !*is_ref {
                            namespace_to_section
                                .insert(label.namespace.clone(), make_section_id(&label.name));
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

/// Assign sanitized names to all of the named labels.
fn assign_named_labels(stats: &mut [ParsedStat], registry: &mut Registry) {
    for stat in stats.iter_mut() {
        for chunk in stat.iter_mut() {
            match chunk {
                Chunk::Label {
                    name,
                    is_ref: _,
                    is_anon: false,
                } => {
                    let mut full_name = CompactString::const_new("");
                    if let Some(namespace) = &name.namespace {
                        full_name.push_str(&namespace);
                        full_name.push('~');
                    }
                    full_name.push_str(&name.name);
                    if let Some(local) = &name.local {
                        full_name.push('.');
                        full_name.push_str(&local);
                    }
                    name.name = registry.sanitize(&full_name).into();
                }
                _ => {}
            }
        }
    }
}

/// Simultaneously:
/// 1. Assign names to anonymous labels.
/// 2. Resolve anonymous backward references to their label names.
fn anonymous_forward_pass(stats: &mut [ParsedStat], registry: &mut Registry, ctx: &Context) {
    // Save generated label names so they can be reused across multiple objects
    let mut label_names = vec![];

    for (stat_index, stat) in stats.iter_mut().enumerate() {
        // Helper: Get the next label name that hasn't been used in this object yet
        let mut i = 0;
        let mut get_next_name = || -> CompactString {
            if i == label_names.len() {
                label_names.push(registry.gen_anonymous());
            }
            let result = label_names[i].clone();
            i += 1;
            result
        };

        // Track each namespace's most recently defined anonymous label
        let mut namespace_to_latest: FxHashMap<Option<CompactString>, CompactString> =
            FxHashMap::default();

        let ctx = ctx.with_stat(stat_index);
        for chunk in stat.iter_mut() {
            match chunk {
                Chunk::Label {
                    is_ref: false,
                    is_anon: true,
                    name,
                } => {
                    let assigned = get_next_name();
                    namespace_to_latest.insert(name.namespace.clone(), assigned.clone());
                    name.name = assigned;
                }
                Chunk::Label {
                    is_ref: true,
                    is_anon: true,
                    name,
                } => {
                    if name.name == "@b" {
                        if let Some(backward) = namespace_to_latest.get(&name.namespace) {
                            name.name = backward.clone();
                        } else {
                            ctx.with_span(name.span.clone())
                                .error("backward reference @b without prior anonymous label :@");
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

/// Resolve anonymous forward references to their label names.
fn anonymous_backward_pass(stats: &mut [ParsedStat], ctx: &Context) {
    for (stat_index, stat) in stats.iter_mut().enumerate() {
        let mut namespace_to_latest = FxHashMap::default();
        let ctx = ctx.with_stat(stat_index);
        for chunk in stat.iter_mut().rev() {
            match chunk {
                Chunk::Label {
                    is_ref: false,
                    is_anon: true,
                    name,
                } => {
                    namespace_to_latest.insert(name.namespace.clone(), name.name.clone());
                }
                Chunk::Label {
                    is_ref: true,
                    is_anon: true,
                    name,
                } => {
                    if name.name == "@f" {
                        if let Some(forward) = namespace_to_latest.get(&name.namespace) {
                            name.name = forward.clone();
                        } else {
                            ctx.with_span(name.span.clone())
                                .error("forward reference @f without following anonymous label :@");
                        }
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

    use crate::{
        error::Context,
        world::{Board, Stat},
    };

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
        let _ = process_labels(&mut board, &Context::default());
        assert_snapshot!(board_to_text(board));
    }

    #[test]
    fn test_anonymous_labels() {
        let mut board = board_from_text("tests/labels/anonymous.txt");
        let _ = process_labels(&mut board, &Context::default());
        assert_snapshot!(board_to_text(board));
    }

    #[test]
    fn test_local_labels() {
        let mut board = board_from_text("tests/labels/local.txt");
        let _ = process_labels(&mut board, &Context::default());
        assert_snapshot!(board_to_text(board));
    }

    #[test]
    fn test_namespaces() {
        let mut board = board_from_text("tests/labels/namespaces.txt");
        let _ = process_labels(&mut board, &Context::default());
        assert_snapshot!(board_to_text(board));
    }
}
