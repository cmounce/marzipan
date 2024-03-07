use byteorder::{ByteOrder, LittleEndian};
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
    info: Vec<u8>,
    code: Vec<u8>,
}

impl World {
    fn from(bytes: &[u8]) -> Result<World, Box<dyn Error>> {
        let mut world = World {
            header: Vec::from(&bytes[0..512]),
            boards: vec![],
        };
        // TODO: Remove LittleEndian entirely?
        let num_boards = 1 + LittleEndian::read_u16(&world.header[2..4]);
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
        for _ in 0..num_stats {
            let mut stat = Stat {
                info: Vec::from(&bytes[offset..offset + 25]),
                code: vec![],
            };
            offset += 25 + 8;
            let code_len = i16::from_le_bytes((&stat.info[23..25]).try_into().unwrap());
            if code_len > 0 {
                let code_len = code_len as usize;
                stat.code = Vec::from(&bytes[offset..offset + code_len]);
                offset += code_len;
            }
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

    println!("num boards: {}", &world.boards.len());
    for board in &world.boards {
        println!("board: {}", to_latin1(&board.name));
        for stat in &board.stats {
            let (x, y) = (stat.info[0] as usize, stat.info[1] as usize);
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
