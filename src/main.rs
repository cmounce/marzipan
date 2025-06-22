mod encoding;
mod labels;
mod peg;
mod preprocess;
mod world;

use anyhow::anyhow;
use labels::process::process_labels;
use preprocess::eval::Context;
use std::{env, error::Error, fs, path::PathBuf, process::exit};
use world::World;

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

    // Codegen: Evaluate all macros
    for board in &mut world.boards {
        for stat in &mut board.stats {
            stat.code = eval_context.eval_program(&stat.code)?;
        }
    }

    // Resolve labels to proper ZZT-OOP
    for mut board in &mut world.boards {
        process_labels(&mut board);
    }

    // Try to write a modified world file
    fs::write("tmp.zzt", world.to_bytes()?)?;

    Ok(())
}
