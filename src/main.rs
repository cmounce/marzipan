mod encoding;
mod labels;
mod preprocess;
mod world;

use anyhow::anyhow;
use encoding::{decode_multiline, encode_multiline};
use preprocess::eval::Context;
use std::{env, error::Error, fs, path::PathBuf, process::exit};
use world::World;

fn to_latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&x| x as char).collect()
}

fn main() -> Result<(), Box<dyn Error>> {
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
        println!("board: {}", to_latin1(&board.name));
        for stat in &mut board.stats {
            // Print some of the data parsed.
            let (x, y) = (stat.x as usize, stat.y as usize);
            println!(
                "  stat at ({}, {}): {:?}, {:?}",
                x,
                y,
                board.terrain[(x - 1) + (y - 1) * 60],
                to_latin1(&stat.code)
            );

            // Evaluate macros
            let old_code = decode_multiline(&stat.code);
            let new_code = eval_context.eval_program(&old_code)?;
            stat.code = encode_multiline(&new_code)?;
        }
    }

    // Try to write a modified world file
    fs::write("tmp.zzt", world.to_bytes()?)?;

    Ok(())
}
