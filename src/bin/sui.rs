// A lot of the initial code from this was taken from the ratatui example, and heavily mutated.
// https://github.com/ratatui-org/ratatui/blob/main/examples/hello_world.rs
//
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use saturn_cli::ui::{layout::draw_loop, state::ProtectedState};
use std::io::{self, Stdout};

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
#[command()]
enum Command {
    #[command(about = "Run the application")]
    Run,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = if std::env::args().len() > 1 {
        ArgParser::parse()
    } else {
        ArgParser {
            command: Command::Run,
        }
    };

    match cli.command {
        Command::Run => run().await?,
    }

    Ok(())
}

async fn run() -> Result<()> {
    let state = ProtectedState::default();

    let s = state.clone();
    tokio::spawn(async move { s.refresh().await });

    let mut terminal = setup_terminal().context("setup failed")?;
    draw_loop(state, &mut terminal)
        .await
        .context("app loop failed")?;
    restore_terminal(&mut terminal).context("restore terminal failed")?;
    Ok(())
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    let mut stdout = io::stdout();
    enable_raw_mode().context("failed to enable raw mode")?;
    execute!(stdout, EnterAlternateScreen).context("unable to enter alternate screen")?;
    Terminal::new(CrosstermBackend::new(stdout)).context("creating terminal failed")
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode().context("failed to disable raw mode")?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)
        .context("unable to switch to main screen")?;
    terminal.show_cursor().context("unable to show cursor")
}
