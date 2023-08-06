use clap::{Parser, Subcommand};
use saturn::cli::{events_now, list_entries, EntryParser};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about)]
#[command(propagate_version = true)]
struct ArgParser {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Notify {
        #[arg(short, long)]
        well: Option<String>,
    },
    Entry {
        args: Vec<String>,
    },
    List {
        #[arg(short, long)]
        all: bool,
    },
    Now {
        #[arg(short, long)]
        well: Option<String>,
    },
}

fn main() -> Result<(), anyhow::Error> {
    let cli = ArgParser::parse();
    match cli.command {
        Command::Notify { well } => {
            let duration = if let Some(well) = well {
                let duration = fancy_duration::FancyDuration::<chrono::Duration>::parse(&well)?;
                duration.duration()
            } else {
                chrono::Duration::seconds(60)
            };

            for entry in events_now(duration)? {
                if let Some(at) = entry.at() {
                    notify_rust::Notification::new()
                        .body(&format!("{} at {}: {}", entry.date(), at, entry.detail()))
                        .show()?;
                } else if let Some(schedule) = entry.scheduled() {
                    notify_rust::Notification::new()
                        .body(&format!(
                            "{} at {} - {}: {}",
                            entry.date(),
                            schedule.0,
                            schedule.1,
                            entry.detail()
                        ))
                        .show()?;
                }
            }
        }
        Command::Now { well } => {
            let duration = if let Some(well) = well {
                let duration = fancy_duration::FancyDuration::<chrono::Duration>::parse(&well)?;
                duration.duration()
            } else {
                chrono::Duration::seconds(60)
            };

            for entry in events_now(duration)? {
                if let Some(at) = entry.at() {
                    println!("{} at {}: {}", entry.date(), at, entry.detail());
                } else if let Some(schedule) = entry.scheduled() {
                    println!(
                        "{} at {} - {}: {}",
                        entry.date(),
                        schedule.0,
                        schedule.1,
                        entry.detail()
                    );
                }
            }
        }
        Command::List { all } => {
            for entry in list_entries(all)? {
                if let Some(at) = entry.at() {
                    println!("{} at {}: {}", entry.date(), at, entry.detail());
                } else if let Some(schedule) = entry.scheduled() {
                    println!(
                        "{} at {} - {}: {}",
                        entry.date(),
                        schedule.0,
                        schedule.1,
                        entry.detail()
                    );
                }
            }
        }
        Command::Entry { args } => {
            let ep = EntryParser::new(args);
            ep.entry()?
        }
    }
    Ok(())
}
