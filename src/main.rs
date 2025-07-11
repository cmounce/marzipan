mod encoding;
mod error;
mod labels;
mod peg;
mod preprocess;
mod world;

use anyhow::{Result, anyhow};
use error::Context as ErrContext;
use labels::process_labels;
use lexopt::prelude::*;
use preprocess::eval::Context;
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::exit,
};
use world::World;

fn main() -> Result<()> {
    let mut input_file = None;
    let mut output_file = None;
    let mut parser = lexopt::Parser::from_env();
    let mut has_args = false;

    while let Some(arg) = parser.next()? {
        has_args = true;
        match arg {
            Short('o') | Long("output") => {
                output_file = Some(parser.value()?.string()?);
            }
            Value(val) => {
                if input_file.is_none() {
                    input_file = Some(val.string()?);
                } else {
                    return Err(anyhow!("Unknown arg: {}", val.to_string_lossy()));
                }
            }
            _ => return Err(arg.unexpected().into()),
        }
    }

    if !has_args {
        eprintln!("Usage: {} INPUT -o OUTPUT", env::args().next().unwrap());
        exit(1);
    }

    let input_filename = input_file.ok_or_else(|| anyhow!("No input file specified"))?;
    let output_filename = output_file.ok_or_else(|| anyhow!("No output file specified"))?;

    // Prevent overwriting input file
    let input_path = Path::new(&input_filename).canonicalize()?;
    let output_path = Path::new(&output_filename);
    let output_dir = output_path
        .parent()
        .filter(|x| !x.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .canonicalize()?;
    let output_path = output_dir.join(output_path.file_name().unwrap());
    if input_path == output_path {
        eprintln!("Error: Output file cannot be the same as input file");
        exit(1);
    }

    let bytes = fs::read(&input_path)?;
    let mut world = World::from_bytes(&bytes)?;

    // Prepare to evaluate macros from the world file's directory
    let world_pathbuf = PathBuf::from(&input_path);
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
    let base_ctx = ErrContext::new();
    let ctx = base_ctx.with_file_path(&input_filename);
    for (i, board) in world.boards.iter_mut().enumerate() {
        let ctx = ctx.with_board(i);
        if let Some(processed_board) = process_labels(&board, &ctx) {
            *board = processed_board;
        }
    }

    // Print diagnostics
    let messages = base_ctx.into_messages();
    for message in messages.iter() {
        println!("{}\n", message.rich_format(&world));
    }
    if !messages.is_empty() {
        let mut warnings = 0;
        let mut errors = 0;
        for message in messages {
            match message.level {
                error::Level::Error => errors += 1,
                error::Level::Warning => warnings += 1,
            }
        }
        let plural = |x| if x == 1 { "" } else { "s" };
        if errors == 0 {
            println!(
                "Compilation succeeded with {warnings} warning{}.",
                plural(warnings)
            )
        } else {
            println!(
                "Compilation failed with {warnings} warning{} and {errors} error{}.",
                plural(warnings),
                plural(errors)
            );
            exit(1);
        }
    }

    // Try to write a modified world file
    let world_bytes = world
        .to_bytes()
        .map_err(|e| anyhow!("Couldn't serialize world, {}", e))?;
    fs::write(&output_path, world_bytes)?;

    Ok(())
}
