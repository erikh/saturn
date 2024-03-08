use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use fancy_duration::FancyDuration;
use saturn_cli::{
    config::{Config, DBType},
    db::{google::GoogleClient, memory::MemoryDB, remote::RemoteDBClient, DB},
    process_cli,
    record::{Record, RecurringRecord, Schedule},
};
use ttygrid::{add_line, grid, header};

macro_rules! compose_grid {
    ($grid:expr, $($header:expr),*) => {{
        use crossterm::style::{Colors, Color};

        let mut grid = grid!($grid, $($header),*).unwrap();
        grid.set_header_color(Colors::new(Color::DarkCyan, Color::Reset));
        grid.set_delimiter_color(Colors::new(Color::Cyan, Color::Reset));
        grid.set_primary_color(Colors::new(Color::White, Color::Reset));
        grid.set_secondary_color(Colors::new(Color::Grey, Color::Reset));

        grid
    }}
}

#[derive(Parser, Debug)]
#[command(
    name = "saturn",
    author = "Erik Hollensbe <git@hollensbe.org>",
    version,
    about = "Control calendars with the CLI"
)]
#[command(propagate_version = true)]
struct ArgParser {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum ConfigCommand {
    #[command(about = "Set the database type you wish to use (unixfile or google)")]
    DBType { db_type: String },
    #[command(about = "Set your client credentials")]
    SetClient {
        client_id: String,
        client_secret: String,
    },
    #[command(about = "Get an authentication token")]
    GetToken {},
    #[command(about = "List Calendar Summaries and their IDs")]
    ListCalendars,
    #[command(about = "Set the calendar ID for remote requests.")]
    SetCalendarID { id: String },
    #[command(about = "Set the default duration for new calendar items that require a range.")]
    SetDefaultDuration { duration: String },
    #[command(about = "Toggle additional helpers for 12h time. False means 'on'.")]
    Set24hTime { set: bool },
    #[command(
        about = "Set the minimum and maximum amount of time to query from the current date for Google Calendar"
    )]
    SetQueryWindow { set: String },
}

#[derive(Debug, Subcommand)]
enum Command {
    #[command(about = "Manipulate Configuration")]
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    #[command(alias = "c", about = "Also `c`. Complete a Task")]
    Complete { id: u64 },
    #[command(
        alias = "d",
        about = "Also `d`. Delete an event by ID. Pass `-r` to delete recurring IDs"
    )]
    Delete {
        ids: Vec<u64>,
        #[arg(short = 'r', long, help = "Delete recurring tasks by ID")]
        recur: bool,
    },
    #[command(about = "Send a visual notification when your appointment has arrived")]
    Notify {
        #[arg(
            short = 'w',
            long,
            help = "Window to consider whether notifying for something"
        )]
        well: Option<String>,
        #[arg(
            short = 't',
            long,
            default_value = "10s",
            help = "Notification timeout"
        )]
        timeout: Option<String>,
        #[arg(short = 'c', long, help = "Include completed tasks (unixfile only)")]
        include_completed: bool,
        #[arg(short = 'i', long, help = "Icon in XDG desktop format")]
        icon: Option<String>,
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
        #[arg(short = 'r', long, help = "List recurring tasks")]
        recur: bool,
        #[arg(short = 'a', long, help = "List all tasks, not just today's")]
        all: bool,
    },
    #[command(
        about = "Edit the details of a specific calendar ID. Use `-r` to specify recurring tasks."
    )]
    Edit {
        #[arg(short = 'r', long, help = "ID is a recurring task")]
        recur: bool,
        id: u64,
    },
    #[command(
        alias = "s",
        about = "Also `s`. Show the details of a specific calendar ID. Use `-r` to specify recurring tasks."
    )]
    Show {
        #[arg(short = 'r', long, help = "ID is a recurring task")]
        recur: bool,
        id: u64,
    },
    #[command(
        alias = "n",
        about = "Also `n`. Show the tasks that are important now, including notifications"
    )]
    Now {
        #[arg(
            short = 'w',
            long,
            help = "Window to consider whether notifying for something"
        )]
        well: Option<String>,
        #[arg(short = 'c', long, help = "Include completed tasks (unixfile only)")]
        include_completed: bool,
    },
    #[command(
        alias = "/",
        about = "Also `/`. Search with terms to identify different calendar items."
    )]
    Search { terms: Vec<String> },
}

fn get_well(well: Option<String>) -> Result<chrono::Duration> {
    if let Some(well) = well {
        Ok(fancy_duration::FancyDuration::<chrono::Duration>::parse(&well)?.duration())
    } else {
        Ok(chrono::TimeDelta::try_seconds(60).unwrap_or_default())
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
        entry.fields().to_string(),
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
        entry.fields().to_string(),
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
        entry.fields().to_string(),
        if entry.completed() { "X" } else { "" }.to_string()
    )
    .unwrap()
}

fn print_entries(entries: Vec<Record>) {
    if entries.is_empty() {
        return;
    }

    let mut grid = compose_grid!(
        header!("TIME", 5),
        header!("DETAIL", 4),
        header!("ID", 6),
        header!("DATE", 3),
        header!("FIELDS", 2),
        header!("DONE", 1)
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

    grid.write(std::io::stdout()).unwrap();
}

fn print_recurring(entries: Vec<RecurringRecord>) {
    if entries.is_empty() {
        return;
    }

    let mut grid = compose_grid!(header!("INTERVAL"), header!("DETAIL"), header!("ID"));

    for mut entry in entries {
        add_line!(
            grid,
            entry.recurrence().to_string(),
            format!(
                "{0:.20}{1}",
                entry.record().detail(),
                if entry.record().detail().len() > 20 {
                    "..."
                } else {
                    ""
                }
            ),
            entry.recurrence_key().to_string()
        )
        .unwrap()
    }

    grid.write(std::io::stdout()).unwrap();
}

fn set_calendar_id(id: String, mut config: Config) -> Result<()> {
    config.set_calendar_id(id);
    config.save(None)
}

async fn list_calendars(mut client: GoogleClient) -> Result<()> {
    let list = client.list_calendars().await?;
    let mut grid = compose_grid!(header!("ID"), header!("SUMMARY"));
    for item in list {
        add_line!(grid, item.id, item.summary).unwrap();
    }
    grid.write(std::io::stdout()).unwrap();
    Ok(())
}

async fn process_google(cli: ArgParser, config: Config) -> Result<()> {
    let client = GoogleClient::new(config.clone())?;

    let mut db = RemoteDBClient::new(config.calendar_id(), client.clone());
    process_cli!(cli, config, db, Some(client.clone()));

    Ok(())
}

async fn process_file(cli: ArgParser, config: Config) -> Result<()> {
    let mut db = MemoryDB::new();
    process_cli!(cli, config, db);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = ArgParser::parse();

    let config = Config::load(None).unwrap_or_default();
    match config.db_type() {
        DBType::UnixFile => process_file(cli, config).await,
        DBType::Google => process_google(cli, config).await,
    }
}
