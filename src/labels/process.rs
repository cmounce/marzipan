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
                Chunk::Label(name) | Chunk::Reference(name) => {
                    let sanitized = registry.sanitize(name);
                    *chunk = Chunk::Verbatim(sanitized.into());
                }
            }
        }
    }

    // Join chunks together and replace old stats' code
    for (old_stat, parsed_stat) in board.stats.iter_mut().zip(stats.into_iter()) {
        let new_code = parsed_stat
            .into_iter()
            .map(|chunk| match chunk {
                Chunk::Verbatim(s) => s,
                _ => unreachable!(),
            })
            .collect();
        old_stat.code = new_code;
    }
}
