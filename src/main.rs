mod encoding;
mod labels;
mod peg;
mod preprocess;
mod world;

use anyhow::anyhow;
use labels::labels::print_labels;
use peg_macro::grammar;
use preprocess::eval::Context;
use std::{env, error::Error, fs, path::PathBuf, process::exit};
use world::World;

fn main() -> Result<(), Box<dyn Error>> {
    grammar!(123);
    grammar!("abc");

    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} WORLD_FILE", args[0]);
        exit(1);
    }

    let world_filename = &args[1];
    let bytes = fs::read(world_filename)?;
    let mut world = World::from_bytes(&bytes)?;

    // Prepare to evaluate macros from the world file's directory
    let world_pathbuf = PathBuf::from(&world_filename);
    let world_dir = world_pathbuf
        .parent()
        .ok_or(anyhow!("Couldn't get world's directory"))?;
    let eval_context = Context::new(&world_dir);

    println!("num boards: {}", &world.boards.len());
    for board in &mut world.boards {
        println!("board: {}", board.name);
        for stat in &mut board.stats {
            // Print some of the data parsed.
            let (x, y) = (stat.x as usize, stat.y as usize);
            let terrain = if x < 1 || x > 60 || y < 1 || y > 25 {
                None
            } else {
                let terrain_index = (x - 1) + (y - 1) * 60;
                Some(board.terrain[terrain_index])
            };
            println!("  stat at ({}, {}): {:?}, {:?}", x, y, terrain, stat.code);

            // Evaluate macros
            stat.code = eval_context.eval_program(&stat.code)?;
        }
        board.name.push_str(" (♪)");
    }

    for board in &world.boards {
        println!("# Board: {}", board.name);
        for stat in &board.stats {
            print_labels(stat);
        }
    }

    // Try to write a modified world file
    fs::write("tmp.zzt", world.to_bytes()?)?;

    Ok(())
}
