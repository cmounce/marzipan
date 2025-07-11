use std::{error::Error, fmt::Display};

use nom::{
    Err, IResult, Parser,
    bytes::complete::{tag, take},
    combinator::fail,
    error::{ErrorKind, ParseError},
    multi::count,
    number::complete::{le_i16, le_u8, le_u16},
};

use crate::encoding::{decode_multiline, decode_oneline, encode_multiline, encode_oneline};

#[derive(Debug)]
pub struct LoadError {
    message: String,
}

impl<I> ParseError<I> for LoadError {
    fn from_error_kind(_input: I, kind: ErrorKind) -> Self {
        Self {
            message: kind.description().into(),
        }
    }

    fn append(_input: I, kind: ErrorKind, other: Self) -> Self {
        Self {
            message: format!("{}: {:?}", other.message, kind),
        }
    }
}

impl From<Err<LoadError>> for LoadError {
    fn from(value: Err<LoadError>) -> Self {
        let message = match value {
            Err::Error(e) => e.message,
            Err::Incomplete(x) => format!("{:?}", x),
            Err::Failure(e) => e.message,
        };
        Self { message }
    }
}

impl From<&str> for LoadError {
    fn from(value: &str) -> Self {
        Self {
            message: value.into(),
        }
    }
}

impl Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.message.fmt(f)
    }
}

impl Error for LoadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }

    fn description(&self) -> &str {
        "description() is deprecated; use Display"
    }

    fn cause(&self) -> Option<&dyn Error> {
        self.source()
    }
}

#[derive(Default)]
pub struct World {
    pub ammo: i16,
    pub gems: i16,
    pub keys: [bool; 7],
    pub health: i16,
    pub starting_board: i16,
    pub torches: i16,
    pub torch_cycles: i16,
    pub energizer_cycles: i16,
    pub score: i16,
    pub world_name: Vec<u8>,
    pub flags: [Vec<u8>; 10],
    pub time: i16,
    pub time_ticks: i16,
    pub locked: bool,
    pub boards: Vec<Board>,
}

#[derive(Clone)]
pub struct Board {
    pub name: String,
    pub terrain: Vec<[u8; 2]>,
    pub max_shots: u8,
    pub is_dark: bool,
    pub board_n: u8,
    pub board_s: u8,
    pub board_w: u8,
    pub board_e: u8,
    pub reenter_when_zapped: bool,
    pub message: Vec<u8>,
    pub enter_x: u8,
    pub enter_y: u8,
    pub time_limit: i16,
    pub stats: Vec<Stat>,
}

#[derive(Clone)]
pub struct Stat {
    pub x: u8,
    pub y: u8,
    pub x_step: i16,
    pub y_step: i16,
    pub cycle: i16,
    pub p1: u8,
    pub p2: u8,
    pub p3: u8,
    pub follower: i16,
    pub leader: i16,
    pub under_element: u8,
    pub under_color: u8,
    pub instruction_pointer: i16,
    pub bind_index: i16,
    pub code: String,
}

impl World {
    pub fn from_bytes(bytes: &[u8]) -> Result<World, LoadError> {
        let (input, (_, num_boards)) = (tag(&[0xff, 0xff][..]), le_i16).parse(bytes)?;
        let (input, (ammo, gems, keys)) =
            (le_i16, le_i16, count(bool_u8, 7)).parse(input)?;
        let (input, (health, starting_board, torches, torch_cycles, energizer_cycles)) =
            (le_i16, le_i16, le_i16, le_i16, le_i16).parse(input)?;
        let (input, (_, score, world_name)) = (take(2usize), le_i16, pstring(20)).parse(input)?;
        let (input, flags) = count(pstring(20), 10).parse(input)?;
        let (_input, (time, time_ticks, locked)) = (le_i16, le_i16, bool_u8).parse(input)?;

        // Rest of header is padding; fast-forward starting from original input
        let (input, _) = take(512usize).parse(bytes)?;

        // Load boards
        let num_boards = num_boards as usize + 1;
        let (_input, chunks) = count(board_slice, num_boards).parse(input)?;
        let boards: Result<Vec<Board>, LoadError> = chunks
            .iter()
            .map(|bytes: &&[u8]| Board::from_bytes(bytes))
            .collect();
        let boards = boards?;

        Ok(World {
            ammo,
            gems,
            keys: keys.try_into().unwrap(),
            health,
            starting_board,
            torches,
            torch_cycles,
            energizer_cycles,
            score,
            world_name,
            flags: flags.try_into().unwrap(),
            time,
            time_ticks,
            locked,
            boards,
        })
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, Box<dyn Error>> {
        let mut result = Vec::with_capacity(512);
        result.push_i16(-1); // file magic: ZZT world
        result.push_i16(self.boards.len() as i16 - 1);
        result.push_i16(self.ammo);
        result.push_i16(self.gems);
        for key in self.keys {
            result.push_bool(key);
        }
        result.push_i16(self.health);
        result.push_i16(self.starting_board);
        result.push_i16(self.torches);
        result.push_i16(self.torch_cycles);
        result.push_i16(self.energizer_cycles);
        result.push_padding(2);
        result.push_i16(self.score);
        result.push_string(20, &self.world_name)?;
        for flag in &self.flags {
            result.push_string(20, flag)?;
        }
        result.push_i16(self.time);
        result.push_i16(self.time_ticks);
        result.push_bool(self.locked);
        result.push_padding(512 - result.len());

        for board in &self.boards {
            result.extend_from_slice(&board.to_bytes()?);
        }
        Ok(result)
    }
}

impl Board {
    pub fn from_bytes(bytes: &[u8]) -> Result<Board, LoadError> {
        // Ignore length bytes
        let (input, _) = le_u16.parse(bytes)?;

        // Read board name
        let (input, name_bytes) = pstring(50)(input)?;
        let name = decode_oneline(&name_bytes);

        // Read terrain
        const NUM_TILES: usize = 60 * 25;
        let mut input = input;
        let mut terrain = Vec::with_capacity(NUM_TILES);
        while terrain.len() < NUM_TILES {
            let (next_input, (count, element, color)) = (le_u8, le_u8, le_u8).parse(input)?;
            input = next_input;
            let count: u32 = if count == 0 { 256 } else { count.into() };
            for _ in 0..count {
                terrain.push([element, color]);
                if terrain.len() > NUM_TILES {
                    return Err("too many tiles of board terrain".into());
                }
            }
        }

        // Read board info
        let (input, (max_shots, is_dark)) = (le_u8, bool_u8).parse(input)?;
        let (input, (board_n, board_s, board_w, board_e)) =
            (le_u8, le_u8, le_u8, le_u8).parse(input)?;
        let (input, (reenter_when_zapped, message)) = (bool_u8, pstring(58)).parse(input)?;
        let (input, (enter_x, enter_y, time_limit)) = (le_u8, le_u8, le_i16).parse(input)?;
        let (input, _) = take(16usize)(input)?;

        // Read stats
        let (input, num_stats) = le_i16(input)?;
        let num_stats = num_stats + 1;
        if num_stats < 0 {
            return Err("cannot have a negative number of stats".into());
        }
        let (_input, stats) = count(Stat::from_bytes, num_stats as usize).parse(input)?;

        Ok(Board {
            name,
            terrain,
            max_shots,
            is_dark,
            board_n,
            board_s,
            board_e,
            board_w,
            reenter_when_zapped,
            message,
            enter_x,
            enter_y,
            time_limit,
            stats,
        })
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, &'static str> {
        let mut result = vec![];
        result.push_padding(2); // reserve space for board size
        let name_bytes = encode_oneline(&self.name).unwrap();
        result.push_string(50, &name_bytes)?;

        // Encode terrain
        if self.terrain.len() != 1500 {
            return Err("invalid number of tiles for board terrain");
        }
        let mut iter = self.terrain.iter().peekable();
        while let Some(tile) = iter.next() {
            let mut count = 1;
            while count < 255 && iter.peek().map_or(false, |&next_tile| next_tile == tile) {
                count += 1;
                iter.next();
            }
            result.push(count);
            result.extend_from_slice(tile);
        }

        // Board info
        result.push(self.max_shots);
        result.push_bool(self.is_dark);
        result.push(self.board_n);
        result.push(self.board_s);
        result.push(self.board_w);
        result.push(self.board_e);
        result.push_bool(self.reenter_when_zapped);
        result.push_string(58, &self.message)?;
        result.push(self.enter_x);
        result.push(self.enter_y);
        result.push_i16(self.time_limit);
        result.push_padding(16);

        // Stats
        let num_stats: i16 = (self.stats.len() - 1)
            .try_into()
            .map_err(|_| "invalid length for stats")?;
        result.push_i16(num_stats);
        for stat in &self.stats {
            result.extend_from_slice(&stat.to_bytes());
        }

        // Fix up board size
        let size: u16 = (result.len() - 2)
            .try_into()
            .map_err(|_| "too many bytes of board data")?;
        result.splice(0..2, size.to_le_bytes());

        Ok(result)
    }
}

impl Stat {
    pub fn from_bytes(input: &[u8]) -> IResult<&[u8], Self, LoadError> {
        let (input, (x, y, x_step, y_step)) = (le_u8, le_u8, le_i16, le_i16).parse(input)?;
        let (input, (cycle, p1, p2, p3)) = (le_i16, le_u8, le_u8, le_u8).parse(input)?;
        let (input, (follower, leader)) = (le_i16, le_i16).parse(input)?;
        let (input, (under_element, under_color)) = (le_u8, le_u8).parse(input)?;
        let (input, _) = take(4usize)(input)?;
        let (input, (instruction_pointer, length)) = (le_i16, le_i16).parse(input)?;
        let (input, _) = take(8usize)(input)?;
        let (input, code_bytes) = take(0.max(length) as usize)(input)?;
        let code = decode_multiline(&code_bytes);
        Ok((
            input,
            Stat {
                x,
                y,
                x_step,
                y_step,
                follower,
                leader,
                cycle,
                p1,
                p2,
                p3,
                under_element,
                under_color,
                instruction_pointer,
                bind_index: 0.min(length),
                code,
            },
        ))
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut result = vec![];
        result.push(self.x);
        result.push(self.y);
        result.push_i16(self.x_step);
        result.push_i16(self.y_step);
        result.push_i16(self.cycle);
        result.push(self.p1);
        result.push(self.p2);
        result.push(self.p3);
        result.push_i16(self.follower);
        result.push_i16(self.leader);
        result.push(self.under_element);
        result.push(self.under_color);
        result.push_padding(4);
        result.push_i16(self.instruction_pointer);
        // TODO: more safety around valid bind-indexes (positive? negative?)
        let code_bytes = encode_multiline(&self.code).unwrap();
        result.push_i16(if self.bind_index < 0 {
            self.bind_index
        } else {
            code_bytes.len() as i16
        });
        result.push_padding(8);
        if self.bind_index >= 0 {
            // TODO: more safety around bind-index XOR code
            result.extend_from_slice(&code_bytes);
        }
        result
    }
}

fn bool_u8(input: &[u8]) -> IResult<&[u8], bool, LoadError> {
    let (input, byte) = le_u8(input)?;
    Ok((input, byte != 0))
}

fn pstring(cap: u8) -> impl Fn(&[u8]) -> IResult<&[u8], Vec<u8>, LoadError> {
    move |input: &[u8]| -> IResult<&[u8], Vec<u8>, LoadError> {
        let (input, len) = le_u8(input)?;
        if len >= cap {
            return fail().parse(input);
        }
        let (input, data) = take(len)(input)?;
        let (input, _) = take(cap - len)(input)?;
        Ok((input, data.to_vec()))
    }
}

fn board_slice(bytes: &[u8]) -> IResult<&[u8], &[u8], LoadError> {
    let (_, size) = le_u16.parse(bytes)?;
    take(size + 2).parse(bytes)
}

trait SerializationHelpers {
    fn push_bool(&mut self, value: bool);
    fn push_i16(&mut self, value: i16);
    fn push_string(&mut self, cap: u8, value: &[u8]) -> Result<(), &'static str>;
    fn push_padding(&mut self, size: usize);
}

impl SerializationHelpers for Vec<u8> {
    fn push_bool(&mut self, value: bool) {
        self.push(if value { 1 } else { 0 });
    }

    fn push_i16(&mut self, value: i16) {
        self.extend(value.to_le_bytes());
    }

    fn push_string(&mut self, cap: u8, value: &[u8]) -> Result<(), &'static str> {
        if value.len() > cap as usize {
            return Err("string too long");
        }
        self.push(value.len() as u8);
        self.extend_from_slice(value);
        self.push_padding(cap as usize - value.len());
        Ok(())
    }

    fn push_padding(&mut self, size: usize) {
        self.resize(self.len() + size, 0);
    }
}
