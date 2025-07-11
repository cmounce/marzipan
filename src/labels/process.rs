use compact_str::CompactString;
use rustc_hash::FxHashMap;

use crate::{error::Context, world::Board};

use super::{
    parse::{Chunk, ParsedStat, parse_stat_labels},
    sanitize::Registry,
};

pub fn process_labels(board: &Board, ctx: &Context) -> Option<Board> {
    let mut board = board.clone();

    // Parse stats into chunks
    let mut stats: Vec<ParsedStat> = board
        .stats
        .iter()
        .enumerate()
        .map(|(index, stat)| parse_stat_labels(&stat, &ctx.with_stat(index)))
        .collect();
    let mut registry = Registry::new();

    // Expand ".local" labels to full "section.local" form.
    resolve_local_labels(&mut stats, ctx);

    // Sanitize all non-anonymous labels.
    // This condenses name strings like "namespace~name$1.local" down to
    // something short and valid for ZZT-OOP, e.g., "local_".
    sanitize_named_labels(&mut stats, &mut registry);

    // Replace anonymous labels with short names.
    // This happens after sanitization so we know which short names are available to use.
    // Two passes are needed because anonymous references can point either forward or backward.
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

    (!ctx.any_errors()).then_some(board)
}

/// Resolve ".local" labels to "name.local" form.
fn resolve_local_labels(stats: &mut [ParsedStat], ctx: &Context) {
    for (i, stat) in stats.iter_mut().enumerate() {
        let ctx = ctx.with_stat(i);

        // Keep a separate resolver for each namespace
        let mut resolvers: FxHashMap<Option<CompactString>, LocalLabelResolver> =
            FxHashMap::default();

        for chunk in stat.iter_mut() {
            match chunk {
                Chunk::Label {
                    is_ref,
                    is_anon: false,
                    name: label,
                } => {
                    let resolver = resolvers.entry(label.namespace.clone()).or_default();
                    let is_definition = !*is_ref;
                    if let Some(local) = &label.local {
                        if label.name.is_empty() {
                            // Local that needs to be resolved, such as ":.foo" or "#send .foo"
                            label.name = resolver.get_section_prefix(&local)
                        } else if is_definition {
                            // Illegal local label definition, such as ":touch.foo"
                            // _References_ to local labels may specify a section name: "#send touch.foo".
                            // But when a local is _defined_, the section name must always be inferred.
                            ctx.with_span(label.span.clone())
                                .error("local label definitions cannot specify a section name");
                        }
                    } else if is_definition {
                        // Top-level label definition, such as ":touch"
                        resolver.start_new_section(&label.name);
                    }
                }
                _ => {}
            }
        }
    }
}

#[derive(Default)]
struct LocalLabelResolver {
    current_section: CompactString,
    current_section_index: usize,
    pair_info: FxHashMap<(CompactString, CompactString), LocalLabelInfo>,
}

struct LocalLabelInfo {
    last_section_index: usize,
    num_sections: usize,
}

impl<'a> LocalLabelResolver {
    /// Record the start of a new section, e.g., `touch`.
    ///
    /// This is called for each occurence of a top-level label; if the same
    /// label name appears multiple times, that creates multiple sections.
    fn start_new_section(&mut self, section: &str) {
        self.current_section = section.into();
        self.current_section_index += 1;
    }

    /// Generate a distinct section string (e.g., `touch$1`) for a local label.
    ///
    /// The output depends not just on the section's and local's names, but also
    /// the number of previous sections that have had that section-local combo:
    ///
    /// - The first time `.foo` appears in a `touch` section, all instances of
    ///     `.foo` within that section resolve to `touch.foo`.
    /// - But if there's a _second_ `touch` label, any instances of `.foo` within
    ///     _that_ section must resolve to a distinct name: `touch$1.foo`.
    fn get_section_prefix(&mut self, local: &str) -> CompactString {
        let key = (self.current_section.clone(), local.into());
        let info = self.pair_info.entry(key).or_insert_with(|| LocalLabelInfo {
            last_section_index: self.current_section_index,
            num_sections: 1,
        });
        if info.last_section_index != self.current_section_index {
            info.last_section_index = self.current_section_index;
            info.num_sections += 1;
        }
        let mut result = self.current_section.clone();
        if info.num_sections > 1 {
            let disambiguator = info.num_sections - 1;
            result.push('$');
            result.push_str(&disambiguator.to_string());
        }
        result
    }
}

/// Assign sanitized names to all of the named labels.
fn sanitize_named_labels(stats: &mut [ParsedStat], registry: &mut Registry) {
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
        let ctx = ctx.with_stat(stat_index);

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
                                .error("backward reference needs an anonymous label");
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
        let ctx = ctx.with_stat(stat_index);

        let mut namespace_to_latest = FxHashMap::default();
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
                                .error("forward reference needs an anonymous label");
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
        world::{Board, Stat, World},
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

    fn world_from_text(path: &str) -> World {
        let mut world = World::default();
        world.boards.push(board_from_text(path));
        world
    }

    fn board_to_text(board: Board) -> String {
        let codes: Vec<_> = board.stats.into_iter().map(|stat| stat.code).collect();
        codes.join("---\n")
    }

    #[test]
    fn test_label_sanitization() {
        let board = board_from_text("tests/labels/sanitize.txt");
        let board = process_labels(&board, &Context::new()).unwrap();
        assert_snapshot!(board_to_text(board));
    }

    #[test]
    fn test_anonymous_labels() {
        let board = board_from_text("tests/labels/anonymous.txt");
        let board = process_labels(&board, &Context::new()).unwrap();
        assert_snapshot!(board_to_text(board));
    }

    #[test]
    fn test_local_labels() {
        let board = board_from_text("tests/labels/local.txt");
        let board = process_labels(&board, &Context::new()).unwrap();
        assert_snapshot!(board_to_text(board));
    }

    #[test]
    fn test_namespaces() {
        let board = board_from_text("tests/labels/namespaces.txt");
        let board = process_labels(&board, &Context::new()).unwrap();
        assert_snapshot!(board_to_text(board));
    }

    #[test]
    fn test_diagnostics() {
        let world = world_from_text("tests/labels/diagnostics.txt");
        let base_ctx = Context::new();
        process_labels(
            &world.boards[0],
            &base_ctx.with_file_path("test.zzt").with_board(0),
        );
        let messages: Vec<String> = base_ctx
            .into_messages()
            .iter()
            .map(|x| x.rich_format(&world))
            .collect();
        assert_snapshot!(messages.join("\n\n"));
    }
}
