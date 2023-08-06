use clap::{Parser, Subcommand};
// use saturn::cli::CLI;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about)]
#[command(propagate_version = true)]
struct ArgParser {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Notify {},
    ShellStatus {},
    Entry {},
}

fn main() -> Result<(), anyhow::Error> {
    let cli = ArgParser::parse();
    eprintln!("{:?}", cli);
    Ok(())
}
