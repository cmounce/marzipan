use std::{cell::RefCell, collections::VecDeque, error::Error, fmt::Display, ops::Range};

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
        // Get base error message
        let message = self.to_string();

        // Build hierarchy string: world -> board -> stat -> span of code
        let mut breadcrumbs = vec![];
        let location = &self.location;
        if let Some(path) = &location.file_path {
            breadcrumbs.push(path.clone());
        }
        let board = location.board.map(|i| &world.boards[i]);
        if let Some(board) = board {
            breadcrumbs.push(board.name.clone());
        }
        let stat = board.and_then(|board| location.stat.map(|i| &board.stats[i]));
        if let Some(stat) = stat {
            let first_line = stat.code.lines().next();
            let name = first_line.filter(|x| x.starts_with("@")).unwrap_or("stat");
            let (x, y) = (stat.x, stat.y);
            breadcrumbs.push(format!("{name} ({x},{y})"));
        }
        let span = stat.and_then(|stat| {
            location
                .span
                .as_ref()
                .map(|span| RichSpan::new(&span, &stat.code))
        });
        if let Some(ref span) = span {
            let line = span.line_number;
            let col = span.line_span.start + 1;
            breadcrumbs.push(format!("line {line}:{col}"))
        }
        let breadcrumbs = format!(" => {}", breadcrumbs.join(" -> "));

        // Build context block
        let context = span.map(|span| {
            let mut block = vec![];

            // Add padding line at start
            let last_line_number = span.nearby_lines.last().unwrap().0;
            let number_width = last_line_number.to_string().len();
            let prefix = format!(" {:number_width$} |", "");
            block.push(prefix.clone());

            // Add each of the context lines
            let mut needs_end_padding = false;
            for (line_number, line) in &span.nearby_lines {
                block.push(format!(" {line_number:>number_width$} | {line}"));
                needs_end_padding = true;

                // Add highlight
                if line_number == &span.line_number {
                    block.push(format!(
                        "{prefix} {}{}",
                        " ".repeat(span.line_span.start),
                        "^".repeat(span.line_span.len())
                    ));
                    needs_end_padding = false;
                }
            }

            // Add padding line at end
            if needs_end_padding {
                block.push(prefix);
            }

            block.join("\n")
        });

        let mut parts = vec![message, breadcrumbs];
        if let Some(context) = context {
            parts.push(context);
        }
        parts.join("\n")
    }
}

struct RichSpan<'a> {
    line_number: usize,
    line_span: Range<usize>,
    nearby_lines: Vec<(usize, &'a str)>,
}

impl<'a> RichSpan<'a> {
    fn new(span: &Range<usize>, code: &'a str) -> Self {
        // Track byte ranges and line numbers for each line
        let mut offset = 0;
        let mut current_line_number = 0;
        let mut lines = code.lines().map(|line| {
            let end_offset = offset + line.len() + 1;
            let range = Range {
                start: offset,
                end: end_offset,
            };
            offset = end_offset;
            current_line_number += 1;
            (range, (current_line_number, line))
        });

        // Find the line where the span starts.
        // While we search, keep track of the immediately preceding lines.
        let num_context_lines = 3;
        let mut recent = VecDeque::with_capacity(num_context_lines * 2 + 1);
        let mut found_line_number = None;
        let mut found_line_span = None;
        for (range, numbered_line) in lines.by_ref() {
            if recent.len() > num_context_lines {
                recent.pop_front();
            }
            recent.push_back(numbered_line);
            if range.contains(&span.start) {
                found_line_number = Some(numbered_line.0);
                let offset = range.start;
                found_line_span = Some(Range {
                    start: span.start - offset,
                    end: span.end - offset,
                });
                break;
            }
        }
        let found_line_number = found_line_number.expect("span outside range of code");
        let found_line_span = found_line_span.unwrap();

        // Gather following context lines
        recent.extend(lines.take(num_context_lines).map(|(_, line)| line));
        recent.make_contiguous();

        Self {
            line_number: found_line_number,
            line_span: found_line_span,
            nearby_lines: recent.into_iter().collect(),
        }
    }
}
