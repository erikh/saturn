use clap::{Parser, Subcommand};
use saturn::cli::EntryParser;

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
    Entry { args: Vec<String> },
    List {},
}

fn main() -> Result<(), anyhow::Error> {
    let cli = ArgParser::parse();
    match cli.command {
        Command::Notify {} => eprintln!("Notify command"),
        Command::ShellStatus {} => eprintln!("ShellStatus command"),
        Command::List {} => eprintln!("List command"),
        Command::Entry { args } => {
            let ep = EntryParser::new(args);
            ep.entry()?
        }
    }
    Ok(())
}
