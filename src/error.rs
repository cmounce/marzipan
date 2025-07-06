use std::{cell::RefCell, error::Error, fmt::Display, ops::Range};

use crate::world::World;

pub enum Context<'a> {
    Base(Box<RefCell<Vec<CompileMessage>>>),
    With(&'a Context<'a>, ContextInfo<'a>),
}

pub enum ContextInfo<'a> {
    FilePath(&'a str),
    Board(usize),
    Stat(usize),
    Span(Range<usize>),
}

impl<'a> Context<'a> {
    pub fn new() -> Self {
        Self::Base(Box::new(RefCell::new(vec![])))
    }

    pub fn with_file_path(&'a self, s: &'a str) -> Self {
        Self::With(self, ContextInfo::FilePath(s))
    }

    pub fn with_board(&'a self, i: usize) -> Self {
        Self::With(self, ContextInfo::Board(i))
    }

    pub fn with_stat(&'a self, i: usize) -> Self {
        Self::With(self, ContextInfo::Stat(i))
    }

    pub fn with_span(&'a self, r: Range<usize>) -> Self {
        Self::With(self, ContextInfo::Span(r))
    }

    fn store(&self, mut message: CompileMessage) {
        match self {
            Context::Base(refcell) => refcell.borrow_mut().push(message),
            Context::With(parent, info) => {
                let location = &mut message.location;
                match info {
                    ContextInfo::FilePath(s) => {
                        location.file_path.get_or_insert((*s).into());
                    }
                    ContextInfo::Board(i) => {
                        location.board.get_or_insert(*i);
                    }
                    ContextInfo::Stat(i) => {
                        location.stat.get_or_insert(*i);
                    }
                    ContextInfo::Span(r) => {
                        location.span.get_or_insert(r.clone());
                    }
                };
                parent.store(message);
            }
        }
    }

    pub fn error(&self, message: &str) {
        self.store(CompileMessage {
            level: Level::Error,
            message: message.into(),
            location: Location::default(),
        });
    }

    pub fn warning(&self, message: &str) {
        self.store(CompileMessage {
            level: Level::Warning,
            message: message.into(),
            location: Location::default(),
        });
    }

    pub fn into_messages(self) -> Vec<CompileMessage> {
        match self {
            Context::Base(refcell) => refcell.into_inner(),
            _ => panic!("into_messages() may only be called on the base context"),
        }
    }
}

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

#[derive(Clone, Debug, Default)]
pub struct Location {
    pub file_path: Option<String>,
    pub board: Option<usize>,
    pub stat: Option<usize>,
    pub span: Option<Range<usize>>,
}

impl Display for CompileMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let level = match self.level {
            Level::Error => "error",
            Level::Warning => "warning",
        };
        write!(f, "{}: {}", level, self.message)
    }
}

impl Error for CompileMessage {}

impl CompileMessage {
    pub fn rich_format(&self, world: &World) -> String {
        format!(
            "{}\n  {}",
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
