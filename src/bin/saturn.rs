use clap::{Parser, Subcommand};
use saturn::{
    cli::{complete_task, delete_event, events_now, list_entries, EntryParser},
    record::{Record, Schedule},
};
use ttygrid::{add_line, grid, header};

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
    #[command(alias = "c", about = "Also `c`. Complete a Task")]
    Complete { id: u64 },
    #[command(alias = "d", about = "Also `d`. Delete an event by ID")]
    Delete { id: u64 },
    #[command(about = "Send a visual notification when your appointment has arrived")]
    Notify {
        #[arg(short = 'w', long)]
        well: Option<String>,
        #[arg(short = 't', long, default_value = "10s")]
        timeout: Option<String>,
    },
    #[command(alias = "e", about = "Also `e`. Enter a new entry into the calendar")]
    Entry { args: Vec<String> },
    #[command(alias = "t", about = "Also `t`. Show today's calendar")]
    Today {},
    #[command(
        alias = "l",
        about = "Also `l`. List today's calendar by default, or --all to show the full calendar"
    )]
    List {
        #[arg(short = 'a', long)]
        all: bool,
    },
    #[command(
        alias = "n",
        about = "Also `n`. Show the tasks that are important now, including notifications"
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

fn grid_at(grid: &mut ttygrid::TTYGrid, entry: Record, at: chrono::NaiveTime) {
    add_line!(
        grid,
        at.to_string(),
        format!(
            "{0:.20}{1}",
            entry.detail(),
            if entry.detail().len() > 20 { "..." } else { "" }
        ),
        entry.primary_key().to_string(),
        entry.date().to_string(),
        if entry.completed() { "X" } else { "" }.to_string()
    )
    .unwrap()
}

fn grid_all_day(grid: &mut ttygrid::TTYGrid, entry: Record) {
    add_line!(
        grid,
        "All Day".to_string(),
        format!(
            "{0:.20}{1}",
            entry.detail(),
            if entry.detail().len() > 20 { "..." } else { "" }
        ),
        entry.primary_key().to_string(),
        entry.date().to_string(),
        if entry.completed() { "X" } else { "" }.to_string()
    )
    .unwrap()
}

fn grid_scheduled(grid: &mut ttygrid::TTYGrid, entry: Record, schedule: Schedule) {
    add_line!(
        grid,
        format!("{} to {}", schedule.0, schedule.1),
        format!(
            "{0:.20}{1}",
            entry.detail(),
            if entry.detail().len() > 20 { "..." } else { "" }
        ),
        entry.primary_key().to_string(),
        entry.date().to_string(),
        if entry.completed() { "X" } else { "" }.to_string()
    )
    .unwrap()
}

fn format_at(entry: Record, at: chrono::NaiveTime) -> String {
    format!(
        "{} at {}: {}{}",
        entry.date(),
        at,
        entry.detail(),
        if entry.completed() { "- Completed" } else { "" }
    )
}

fn format_scheduled(entry: Record, schedule: Schedule) -> String {
    format!(
        "{} at {} to {}: {}{}",
        entry.date(),
        schedule.0,
        schedule.1,
        entry.detail(),
        if entry.completed() { "- Completed" } else { "" }
    )
}

fn print_entries(entries: Vec<Record>) {
    if entries.is_empty() {
        return;
    }

    let mut grid = grid!(
        header!("TIME"),
        header!("DETAIL"),
        header!("ID"),
        header!("DATE"),
        header!("DONE")
    );

    for entry in entries {
        if let Some(at) = entry.at() {
            grid_at(&mut grid, entry, at);
        } else if let Some(schedule) = entry.scheduled() {
            grid_scheduled(&mut grid, entry, schedule);
        } else if entry.all_day() {
            grid_all_day(&mut grid, entry);
        }
    }

    println!("{}", grid.display().unwrap());
}

fn main() -> Result<(), anyhow::Error> {
    let cli = ArgParser::parse();
    match cli.command {
        Command::Complete { id } => complete_task(id)?,
        Command::Delete { id } => delete_event(id)?,
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
