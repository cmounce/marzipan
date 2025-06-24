use std::{error::Error, fmt::Display, ops::Range};

use crate::world::World;

pub type CompileResult = Result<Vec<CompileMessage>, Vec<CompileMessage>>;

#[derive(Debug)]
pub struct CompileMessage {
    pub level: Level,
    pub message: String,
    pub location: Location,
}

#[derive(Debug)]
pub enum Level {
    Error,
    Warning,
}

#[derive(Debug)]
pub struct Location {
    pub file_path: Option<String>,
    pub board: Option<usize>,
    pub stat: Option<usize>,
    pub span: Option<Range<usize>>,
}

impl Display for CompileMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let level = match self.level {
            Level::Error => "Error",
            Level::Warning => "Warning",
        };
        write!(f, "{}: {}", level, self.message)
    }
}

impl Error for CompileMessage {}

impl CompileMessage {
    pub fn rich_format(&self, world: &World) -> String {
        format!(
            "{}\n{}",
            self.to_string(),
            self.location.rich_format(&world)
        )
    }
}

impl Location {
    pub fn rich_format(&self, world: &World) -> String {
        let board = self.board.map(|i| &world.boards[i]);
        let stat = self.stat.and_then(|i| board.map(|b| &b.stats[i]));

        let mut parts: Vec<Option<String>> = vec![];
        parts.push(self.file_path.clone());
        parts.push(board.map(|board| board.name.clone()));
        parts.push(stat.map(|stat| format!("stat({},{})", stat.x, stat.y)));
        parts.push(self.span.as_ref().and_then(|span| {
            stat.map(|stat| {
                let mut line = 1;
                let mut col = 1;
                for c in (&stat.code[0..span.start]).chars() {
                    if c == '\n' {
                        line += 1;
                        col = 1;
                    } else {
                        col += 1;
                    }
                }
                format!("line {line}, column {col}")
            })
        }));

        while parts.len() > 0 && parts[parts.len() - 1].is_none() {
            parts.pop();
        }
        let parts: Vec<_> = parts
            .into_iter()
            .map(|x| x.unwrap_or("???".into()))
            .collect();
        parts.join(", ")
    }
}
