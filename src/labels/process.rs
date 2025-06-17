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
                    let sanitized = registry.sanitize(name);
                    *chunk = Chunk::Verbatim(sanitized.into());
                }
            }
        }
    }

    anonymous_forward_pass(&mut stats, &mut registry);
    anonymous_backward_pass(&mut stats);

    // Join chunks together and replace old stats' code
    for (old_stat, parsed_stat) in board.stats.iter_mut().zip(stats.into_iter()) {
        let new_code = parsed_stat
            .into_iter()
            .map(|chunk| match chunk {
                Chunk::Verbatim(s) => s,
                _ => unimplemented!(),
            })
            .collect();
        old_stat.code = new_code;
    }
}

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
