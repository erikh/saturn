use clap::{Parser, Subcommand};
use console::style;
use saturn::{
    cli::{events_now, list_entries, EntryParser},
    record::{Record, Schedule},
};

#[derive(Parser, Debug)]
#[command(
    author = "Erik Hollensbe <erik+github@hollensbe.org>",
    version,
    about = "Control calendars with the CLI"
)]
#[command(propagate_version = true)]
struct ArgParser {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    #[command(about = "Send a visual notification when your appointment has arrived")]
    Notify {
        #[arg(short = 'w', long)]
        well: Option<String>,
        #[arg(short = 't', long)]
        timeout: Option<String>,
    },
    #[command(about = "Enter a new entry into the calendar")]
    Entry { args: Vec<String> },
    #[command(alias = "t", about = "Show today's calendar")]
    Today {},
    #[command(about = "List today's calendar by default, or --all to show the full calendar")]
    List {
        #[arg(short = 'a', long)]
        all: bool,
    },
    #[command(
        alias = "n",
        about = "Show the tasks that are important now, including notifications"
    )]
    Now {
        #[arg(short = 'w', long)]
        well: Option<String>,
    },
}

fn get_well(well: Option<String>) -> Result<chrono::Duration, anyhow::Error> {
    if let Some(well) = well {
        Ok(fancy_duration::FancyDuration::<chrono::Duration>::parse(&well)?.duration())
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

fn print(line: String, shade: bool) {
    if shade {
        println!("{}", style(line).dim().white())
    } else {
        println!("{}", style(line).white())
    }
}

fn print_entries(entries: Vec<Record>) {
    let mut shade = false;

    for entry in entries {
        if let Some(at) = entry.at() {
            print(format_at(entry, at), shade)
        } else if let Some(schedule) = entry.scheduled() {
            print(format_scheduled(entry, schedule), shade)
        }
        shade = !shade;
    }
}

fn main() -> Result<(), anyhow::Error> {
    let cli = ArgParser::parse();
    match cli.command {
        Command::Notify { well, timeout } => {
            let timeout = timeout.map_or(std::time::Duration::new(60, 0), |t| {
                fancy_duration::FancyDuration::<std::time::Duration>::parse(&t)
                    .expect("Invalid Duration")
                    .duration()
            });

            let mut notification = notify_rust::Notification::new();
            notification.summary("Calendar Event");
            notification.timeout(timeout);

            for entry in events_now(get_well(well)?)? {
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
            print_entries(events_now(get_well(well)?)?);
        }
        Command::List { all } => {
            print_entries(list_entries(all)?);
        }
        Command::Today {} => {
            print_entries(list_entries(false)?);
        }
        Command::Entry { args } => {
            EntryParser::new(args).entry()?;
        }
    }
    Ok(())
}
