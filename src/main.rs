mod lang;
mod world;

use std::{env, error::Error, fs, process::exit};
use world::World;

use crate::lang::scan;

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
    let mut world = World::from_bytes(&bytes)?;

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
            scan("todo: convert [u8] to strings and scan here");
        }
    }

    // Try to write a modified world file
    world.boards.reverse();
    world.starting_board = (world.boards.len() as i16 - 1) - world.starting_board;
    fs::write("tmp.zzt", world.to_bytes()?)?;

    Ok(())
}
