use nom::{
    bytes::complete::take,
    number::complete::{le_i16, le_u8},
    sequence::tuple,
    IResult,
};
use std::{env, error::Error, fs, process::exit};

trait SerializationHelpers {
    fn push_i16(&mut self, value: i16);
    fn push_u16(&mut self, value: u16);
    fn push_string(&mut self, cap: u8, value: &[u8]) -> Result<(), &'static str>;
    fn push_padding(&mut self, size: usize);
}

impl SerializationHelpers for Vec<u8> {
    fn push_i16(&mut self, value: i16) {
        self.extend(value.to_le_bytes());
    }

    fn push_u16(&mut self, value: u16) {
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

struct World {
    header: Vec<u8>,
    boards: Vec<Board>,
}

struct Board {
    name: Vec<u8>,
    terrain: Vec<[u8; 2]>,
    info: Vec<u8>,
    stats: Vec<Stat>,
}

struct Stat {
    x: u8,
    y: u8,
    x_step: i16,
    y_step: i16,
    cycle: i16,
    p1: u8,
    p2: u8,
    p3: u8,
    follower: i16,
    leader: i16,
    under_element: u8,
    under_color: u8,
    instruction_pointer: i16,
    bind_index: i16,
    code: Vec<u8>,
}

impl World {
    fn from(bytes: &[u8]) -> Result<World, Box<dyn Error>> {
        let mut world = World {
            header: Vec::from(&bytes[0..512]),
            boards: vec![],
        };
        let num_boards = 1 + u16::from_le_bytes((&world.header[2..4]).try_into().unwrap());
        let mut offset = world.header.len();
        for _ in 0..num_boards {
            // Ideally, this wouldn't panic if we run out of bytes
            let board_len =
                u16::from_le_bytes((&bytes[offset..offset + 2]).try_into().unwrap()) as usize;
            offset += 2;
            world
                .boards
                .push(Board::from(&bytes[offset..offset + board_len])?);
            offset += board_len;
        }
        Ok(world)
    }
}

impl Board {
    fn from(bytes: &[u8]) -> Result<Board, Box<dyn Error>> {
        // Read board name
        let name_len = bytes[0] as usize;
        let name = Vec::from(&bytes[1..name_len + 1]);
        let mut offset = 51; // skip Pascal string[50]

        // Read terrain
        let mut terrain = vec![[0; 2]; 60 * 25];
        let mut i = 0;
        while i < terrain.len() {
            let times = if bytes[offset] == 0 {
                256
            } else {
                bytes[offset] as usize
            };
            let tile = (&bytes[offset + 1..offset + 3]).try_into().unwrap();
            offset += 3;
            for _ in 0..times {
                terrain[i] = tile;
                i += 1
            }
        }

        // Read board info
        let info = Vec::from(&bytes[offset..offset + 86]);
        offset += 86;

        // Read stats
        let num_stats =
            u16::from_le_bytes((&bytes[offset..offset + 2]).try_into().unwrap()) as usize + 1;
        offset += 2;
        let mut stats = Vec::with_capacity(num_stats);
        let mut input = &bytes[offset..];
        for _ in 0..num_stats {
            let (next_input, stat) = Stat::parse(input).map_err(|e| e.to_owned())?;
            input = next_input;
            stats.push(stat);
        }
        Ok(Board {
            name,
            terrain,
            info,
            stats,
        })
    }

    fn to_bytes(&self) -> Result<Vec<u8>, &'static str> {
        let mut result = vec![];
        result.push_padding(2); // reserve space for board size
        result.push_string(50, &self.name)?;

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
        result.extend_from_slice(&self.info);

        // Stats
        // TODO: handle when there are no stats
        result.push_i16((self.stats.len() - 1) as i16);
        for stat in &self.stats {
            result.extend_from_slice(&stat.to_bytes());
        }

        // Fix up board size
        let size = (result.len() - 2) as u16;
        result.splice(0..2, size.to_le_bytes());

        Ok(result)
    }
}

impl Stat {
    fn parse(input: &[u8]) -> IResult<&[u8], Self> {
        let (input, (x, y, x_step, y_step)) = tuple((le_u8, le_u8, le_i16, le_i16))(input)?;
        let (input, (cycle, p1, p2, p3)) = tuple((le_i16, le_u8, le_u8, le_u8))(input)?;
        let (input, (follower, leader)) = tuple((le_i16, le_i16))(input)?;
        let (input, (under_element, under_color)) = tuple((le_u8, le_u8))(input)?;
        let (input, _) = take(4usize)(input)?;
        let (input, (instruction_pointer, length)) = tuple((le_i16, le_i16))(input)?;
        let (input, _) = take(8usize)(input)?;
        let (input, code) = take(0.max(length) as usize)(input)?;
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
                code: Vec::from(code),
            },
        ))
    }

    fn to_bytes(&self) -> Vec<u8> {
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
        result.push_i16(if self.bind_index < 0 {
            self.bind_index
        } else {
            self.code.len() as i16
        });
        result.push_padding(8);
        if self.bind_index >= 0 {
            // TODO: more safety around bind-index XOR code
            result.extend_from_slice(&self.code);
        }
        result
    }
}

fn to_latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&x| x as char).collect()
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} WORLD_FILE", args[0]);
        exit(1);
    }

    let bytes = fs::read(&args[1])?;
    let world = World::from(&bytes)?;

    // Print some of the data parsed.
    // TODO: Implement some macros, or other code modification
    // Then, we can try writing a copy to disk.
    println!("num boards: {}", &world.boards.len());
    for board in &world.boards {
        println!("board: {}", to_latin1(&board.name));
        for stat in &board.stats {
            let (x, y) = (stat.x as usize, stat.y as usize);
            println!(
                "  stat at ({}, {}): {:?}, {:?}",
                x,
                y,
                board.terrain[(x - 1) + (y - 1) * 60],
                to_latin1(&stat.code)
            );
        }
    }

    // Try to export one of the boards
    fs::write("tmp.brd", world.boards[0].to_bytes()?)?;

    Ok(())
}
