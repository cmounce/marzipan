use nom::{
    bytes::complete::take,
    number::complete::{le_i16, le_u8},
    sequence::tuple,
    IResult,
};
use std::{env, error::Error, fs, process::exit};

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
    Ok(())
}
