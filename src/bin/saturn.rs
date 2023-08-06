use clap::{Parser, Subcommand};
use saturn::{
    cli::{events_now, list_entries, EntryParser},
    record::{Record, Schedule},
};

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
        #[arg(short, long)]
        timeout: Option<String>,
    },
    Entry {
        args: Vec<String>,
    },
    Today {},
    List {
        #[arg(short, long)]
        all: bool,
    },
    Now {
        #[arg(short, long)]
        well: Option<String>,
    },
}

fn get_well(well: Option<String>) -> Result<chrono::Duration, anyhow::Error> {
    if let Some(well) = well {
        let duration = fancy_duration::FancyDuration::<chrono::Duration>::parse(&well)?;
        Ok(duration.duration())
    } else {
        Ok(chrono::Duration::seconds(60))
    }
}

fn format_at(entry: Record, at: chrono::NaiveTime) -> String {
    format!("{} at {}: {}", entry.date(), at, entry.detail())
}

fn format_scheduled(entry: Record, schedule: Schedule) -> String {
    format!(
        "{} at {} - {}: {}",
        entry.date(),
        schedule.0,
        schedule.1,
        entry.detail()
    )
}

fn print(line: String) {
    println!("{}", line)
}

fn main() -> Result<(), anyhow::Error> {
    let cli = ArgParser::parse();
    match cli.command {
        Command::Notify { well, timeout } => {
            let duration = get_well(well)?;
            let timeout = timeout.map_or(std::time::Duration::new(60, 0), |t| {
                fancy_duration::FancyDuration::<std::time::Duration>::parse(&t)
                    .expect("Invalid Duration")
                    .duration()
            });

            let mut notification = notify_rust::Notification::new();
            notification.summary("Calendar Event");
            notification.timeout(timeout);

            for entry in events_now(duration)? {
                if let Some(at) = entry.at() {
                    notification.body(&format_at(entry, at)).show()?;
                } else if let Some(schedule) = entry.scheduled() {
                    notification
                        .body(&format_scheduled(entry, schedule))
                        .show()?;
                }
            }
        }
        Command::Now { well } => {
            let duration = get_well(well)?;

            for entry in events_now(duration)? {
                if let Some(at) = entry.at() {
                    print(format_at(entry, at))
                } else if let Some(schedule) = entry.scheduled() {
                    print(format_scheduled(entry, schedule))
                }
            }
        }
        Command::List { all } => {
            for entry in list_entries(all)? {
                if let Some(at) = entry.at() {
                    print(format_at(entry, at))
                } else if let Some(schedule) = entry.scheduled() {
                    print(format_scheduled(entry, schedule))
                }
            }
        }
        Command::Today {} => {
            for entry in list_entries(false)? {
                if let Some(at) = entry.at() {
                    print(format_at(entry, at))
                } else if let Some(schedule) = entry.scheduled() {
                    print(format_scheduled(entry, schedule))
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
