use anyhow::anyhow;
use clap::{Parser, Subcommand};
use fancy_duration::FancyDuration;
use saturn_cli::{
    cli::{
        get_access_token, get_config, set_client_info, set_db_type, set_sync_window, EntryParser,
    },
    config::{Config, DBType},
    db::{google::GoogleClient, memory::MemoryDB, remote::RemoteDBClient, DB},
    process_cli,
    record::{Record, RecurringRecord, Schedule},
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
    #[command(
        about = "Set the synchronization window for remote requests. Window will be both added to the leading and trailing duration."
    )]
    SetSyncWindow { window: String },
    #[command(about = "List Calendar Summaries and their IDs")]
    ListCalendars,
    #[command(about = "Set the calendar ID for remote requests.")]
    SetCalendarID { id: String },
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
        id: u64,
        #[arg(short = 'r', long)]
        recur: bool,
    },
    #[command(about = "Send a visual notification when your appointment has arrived")]
    Notify {
        #[arg(short = 'w', long)]
        well: Option<String>,
        #[arg(short = 't', long, default_value = "10s")]
        timeout: Option<String>,
        #[arg(short = 'c', long)]
        include_completed: bool,
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
        #[arg(short = 'r', long)]
        recur: bool,
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
        #[arg(short = 'c', long)]
        include_completed: bool,
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

fn print_recurring(entries: Vec<RecurringRecord>) {
    if entries.is_empty() {
        return;
    }

    let mut grid = grid!(header!("INTERVAL"), header!("DETAIL"), header!("ID"));

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

    println!("{}", grid.display().unwrap());
}

fn set_calendar_id(id: String, mut config: Config) -> Result<(), anyhow::Error> {
    config.set_calendar_id(id);
    config.save(None)
}

async fn list_calendars(mut client: GoogleClient) -> Result<(), anyhow::Error> {
    let list = client.list_calendars().await?;
    let mut grid = grid!(header!("ID"), header!("SUMMARY"));
    for item in list {
        add_line!(grid, item.id, item.summary).unwrap();
    }
    println!("{}", grid.display().unwrap());
    Ok(())
}

async fn process_google(cli: ArgParser, config: Config) -> Result<(), anyhow::Error> {
    let client = GoogleClient::new(config.clone())?;

    let mut db = RemoteDBClient::new(config.calendar_id(), client.clone());
    process_cli!(cli, config, db, Some(client.clone()));

    Ok(())
}

async fn process_file(cli: ArgParser, config: Config) -> Result<(), anyhow::Error> {
    let mut db = MemoryDB::new();
    process_cli!(cli, config, db);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let cli = ArgParser::parse();

    let config = get_config().unwrap_or_default();
    match config.db_type() {
        DBType::UnixFile => process_file(cli, config).await,
        DBType::Google => process_google(cli, config).await,
    }
}
