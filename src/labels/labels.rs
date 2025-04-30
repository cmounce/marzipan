use super::super::preprocess::peg::Parser;
use anyhow::Result;

fn parse_program(p: &mut Parser) -> Result<()> {
    p.star(|p| parse_line(p))?;
    p.eof()?;
    Ok(())
}

fn parse_line(p: &mut Parser) -> Result<()> {
    p.star(|p| p.char(|c| c != '\n'))?;
    p.literal("\n")?;
    Ok(())
}
