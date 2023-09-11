use crate::{
    record::Record,
    time::now,
    ui::{
        consts::*,
        state::{ProtectedState, State},
        types::*,
    },
};
use chrono::Datelike;
use crossterm::event::{self, Event, KeyCode};
use ratatui::{prelude::*, widgets::*};
use std::time::Duration;
use std::{io::Stdout, ops::Deref, sync::Arc};

fn sit<T>(
    msg: impl std::future::Future<Output = Result<T, anyhow::Error>>,
) -> Result<T, anyhow::Error> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    runtime.block_on(msg)
}

pub async fn draw_loop<'a>(
    state: ProtectedState<'static>,
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
) -> Result<(), anyhow::Error> {
    let (s, mut r) = tokio::sync::mpsc::channel(1);

    let s2 = state.clone();
    std::thread::spawn(move || sit(read_input(s2, s)));
    let mut last_line = String::from("placeholder");
    let mut last_draw = now() - chrono::Duration::minutes(1);

    loop {
        let mut lock = state.lock().await;
        let redraw = lock.redraw;

        if redraw {
            lock.redraw = false;
        }

        let line = lock.line_buf.clone();
        drop(lock);

        if redraw || line != last_line || last_draw + chrono::Duration::seconds(5) < now() {
            terminal.draw(|f| {
                render_app(state.clone(), f, line.clone());
            })?;

            last_line = line;
            last_draw = now();
        }

        if r.try_recv().is_ok() {
            break;
        }

        tokio::time::sleep(Duration::new(0, 100)).await;
    }
    Ok(())
}

pub async fn read_input<'a>(
    state: ProtectedState<'static>,
    s: tokio::sync::mpsc::Sender<()>,
) -> Result<(), anyhow::Error> {
    'input: loop {
        let mut buf = state.lock().await.line_buf.clone();
        buf = handle_input(buf).expect("Invalid input");
        if buf.ends_with('\n') {
            match buf.trim() {
                "quit" => break 'input,
                x => {
                    if x.starts_with("s ") || x.starts_with("show ") {
                        let m = if x.starts_with("show ") {
                            x.trim_start_matches("show ")
                        } else {
                            x.trim_start_matches("s ")
                        };
                        match m {
                            "all" | "a" => {
                                state.lock().await.list_type = ListType::All;
                                let state = state.clone();
                                tokio::spawn(async move {
                                    state.add_notification("Updating state").await;
                                    match state.update_state().await {
                                        Ok(_) => {}
                                        Err(e) => {
                                            state.add_notification(&format!("Error: {}", e)).await;
                                        }
                                    }
                                });
                            }
                            "today" | "t" => {
                                state.lock().await.list_type = ListType::Today;
                                let state = state.clone();
                                tokio::spawn(async move {
                                    state.add_notification("Updating state").await;
                                    match state.update_state().await {
                                        Ok(_) => {}
                                        Err(e) => {
                                            state.add_notification(&format!("Error: {}", e)).await;
                                        }
                                    }
                                });
                            }
                            "recur" | "recurring" | "recurrence" | "r" => {
                                state.lock().await.list_type = ListType::Recurring;
                                let state = state.clone();
                                tokio::spawn(async move {
                                    state.add_notification("Updating state").await;
                                    match state.update_state().await {
                                        Ok(_) => {}
                                        Err(e) => {
                                            state.add_notification(&format!("Error: {}", e)).await;
                                        }
                                    }
                                });
                            }
                            _ => {
                                state
                                    .add_notification(&format!("Invalid Command '{}'", x))
                                    .await
                            }
                        }
                    } else if x.starts_with("d ") || x.starts_with("delete ") {
                        let ids = if x.starts_with("delete ") {
                            x.trim_start_matches("delete ")
                        } else {
                            x.trim_start_matches("d ")
                        }
                        .split(' ')
                        .filter(|x| !x.is_empty())
                        .collect::<Vec<&str>>();

                        let mut v = Vec::new();

                        for id in &ids {
                            if id.is_empty() || *id == "recur" {
                                continue;
                            }
                            match id.parse::<u64>() {
                                Ok(y) => v.push(y),
                                Err(_) => {
                                    state.add_notification(&format!("Invalid ID {}", id)).await;
                                }
                            };
                        }

                        let command = if !ids.is_empty() && ids[0] == "recur" && ids.len() > 1 {
                            CommandType::DeleteRecurring(v[1..v.len()].to_vec())
                        } else {
                            CommandType::Delete(v)
                        };

                        let state = state.clone();
                        tokio::spawn(async move {
                            state.lock().await.command = Some(command);
                            state.add_notification("Updating state").await;
                            match state.update_state().await {
                                Ok(_) => {}
                                Err(e) => {
                                    state.add_notification(&format!("Error: {}", e)).await;
                                }
                            }
                        });
                    } else if x.starts_with("e ") || x.starts_with("entry ") {
                        let x = x.to_string();

                        let state = state.clone();
                        tokio::spawn(async move {
                            state.lock().await.command = Some(CommandType::Entry(
                                if x.starts_with("entry ") {
                                    x.trim_start_matches("entry ")
                                } else {
                                    x.trim_start_matches("e ")
                                }
                                .to_string(),
                            ));
                            state.add_notification("Updating state").await;
                            match state.update_state().await {
                                Ok(_) => {}
                                Err(e) => {
                                    state.add_notification(&format!("Error: {}", e)).await;
                                }
                            }
                        });
                    } else {
                        state.add_notification("Invalid Command").await;
                    }
                }
            }
            buf = String::new();
        }
        state.lock().await.line_buf = buf;
        tokio::time::sleep(Duration::new(0, 50)).await;
    }
    s.send(()).await?;
    Ok(())
}

pub fn render_app(
    state: ProtectedState<'static>,
    frame: &mut ratatui::Frame<'_, CrosstermBackend<Stdout>>,
    buf: String,
) {
    let layout = Layout::default()
        .constraints([Constraint::Length(1), Constraint::Percentage(100)].as_ref())
        .split(frame.size());

    let line_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(80), Constraint::Percentage(20)].as_ref())
        .split(layout[0]);

    let draw_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)].as_ref())
        .split(layout[1]);

    let s = state.clone();
    let calendar = std::thread::spawn(move || sit(build_calendar(s)))
        .join()
        .expect("could not build calendar")
        .expect("could not build calendar");

    let s = state.clone();
    let events = std::thread::spawn(move || sit(build_events(s)))
        .join()
        .expect("could not build events")
        .expect("could not build events");

    let s = state.clone();
    let notification = std::thread::spawn(move || {
        sit(async move {
            let mut lock = s.lock().await;
            let ret = lock.notification.clone();

            if let Some(ret) = &ret {
                if now().naive_local() > ret.1 + chrono::Duration::seconds(1) {
                    lock.notification = None;
                }
            }

            Ok(ret)
        })
    })
    .join()
    .expect("could not get notification")
    .expect("could not get notification");

    if let Some(notification) = notification {
        frame.render_widget(Paragraph::new(format!(">> {}", buf)), line_layout[0]);
        frame.render_widget(
            Paragraph::new(format!("[ {} ]", notification.0)),
            line_layout[1],
        );
    } else {
        frame.render_widget(Paragraph::new(format!(">> {}", buf)), layout[0]);
    }

    frame.render_widget(calendar.deref().clone(), draw_layout[0]);
    frame.render_widget(events.deref().clone(), draw_layout[1]);
    frame.set_cursor(3 + buf.len() as u16, 0);
}

pub async fn build_calendar<'a>(
    state: ProtectedState<'static>,
) -> Result<Arc<Table<'a>>, anyhow::Error> {
    if let Some(calendar) = state.lock().await.calendar.clone() {
        if calendar.1 + chrono::Duration::seconds(1) > now().naive_local() {
            return Ok(calendar.0);
        }
    }

    let header_cells = ["", "Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat", ""]
        .iter()
        .map(|h| Cell::from(*h).style(*TITLE_STYLE));
    let header = Row::new(header_cells)
        .style(*HEADER_STYLE)
        .height(1)
        .bottom_margin(1);

    let mut rows = Vec::new();
    let mut last_row: Vec<(Cell<'_>, usize)> = Vec::new();
    last_row.push((Cell::from("".to_string()), 0));

    let datetime = now();
    let date = now().date_naive();
    let mut begin = chrono::NaiveDateTime::new(
        chrono::NaiveDate::from_ymd_opt(
            date.year_ce().1 as i32,
            date.month0() + 1,
            (date - chrono::Duration::days(datetime.weekday().num_days_from_sunday().into()))
                .day0()
                + 1,
        )
        .unwrap(),
        chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
    );

    let mut lock = state.lock().await;
    for x in 0..DAYS {
        if x % DAYS_IN_WEEK == 0 && x != 0 {
            last_row.push((Cell::from("".to_string()), 0));
            rows.push(
                Row::new(last_row.iter().map(|x| x.0.clone()).collect::<Vec<Cell>>()).height({
                    let res = last_row.iter().map(|res| res.1).max().unwrap_or(4) as u16;
                    if res > 4 {
                        res
                    } else {
                        4
                    }
                }),
            );
            rows.push(Row::new(
                ["", "", "", "", "", "", "", "", ""].map(Cell::from),
            ));
            last_row = Vec::new();
            last_row.push((Cell::from("".to_string()), 0));
        }

        last_row.push(build_data(&mut lock, begin).await);
        begin += chrono::Duration::days(1);
    }
    drop(lock);
    last_row.push((Cell::from("".to_string()), 0));
    rows.push(
        Row::new(last_row.iter().map(|x| x.0.clone()).collect::<Vec<Cell>>()).height({
            let res = last_row.iter().map(|x| x.1).max().unwrap_or(4) as u16;
            if res > 4 {
                res as u16
            } else {
                4
            }
        }),
    );

    let table = Arc::new(
        Table::new(rows.clone())
            .header(header)
            .block(
                Block::default().borders(Borders::ALL).title(
                    chrono::Month::try_from(now().month() as u8)
                        .expect("Invalid Month")
                        .name(),
                ),
            )
            .widths(&[
                Constraint::Percentage(3),
                Constraint::Percentage(12),
                Constraint::Percentage(12),
                Constraint::Percentage(12),
                Constraint::Percentage(12),
                Constraint::Percentage(12),
                Constraint::Percentage(12),
                Constraint::Percentage(12),
                Constraint::Percentage(3),
            ]),
    );

    let mut lock = state.lock().await;

    if (!rows.is_empty() && lock.calendar.is_none()) || lock.calendar.is_some() {
        lock.calendar = Some((table.clone(), now().naive_local()));
    }

    Ok(table)
}

pub async fn build_events<'a>(
    state: ProtectedState<'static>,
) -> Result<Arc<Table<'a>>, anyhow::Error> {
    if let Some(events) = state.lock().await.events.clone() {
        if events.1 + chrono::Duration::seconds(1) > now().naive_local() {
            return Ok(events.0);
        }
    }

    let header_cells = ["ID", "Time", "Summary"]
        .iter()
        .map(|h| Cell::from(*h).style(*TITLE_STYLE));
    let header = Row::new(header_cells)
        .style(*HEADER_STYLE)
        .height(1)
        .bottom_margin(1);

    let datetime = now();
    let date = datetime.date_naive();
    let begin = chrono::NaiveDateTime::new(
        chrono::NaiveDate::from_ymd_opt(
            date.year_ce().1 as i32,
            date.month0() + 1,
            (date - chrono::Duration::days(datetime.weekday().num_days_from_sunday().into()))
                .day0()
                + 1,
        )
        .unwrap(),
        chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
    );

    let mut inner = state.lock().await;
    let rows = match inner.list_type {
        ListType::All | ListType::Today => inner
            .records
            .iter()
            .filter_map(|r| {
                if (r.all_day()
                    && chrono::NaiveDateTime::new(
                        r.date(),
                        chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
                    ) > begin)
                    || r.datetime().naive_local() > begin
                {
                    let mut row = Row::new(vec![
                        Cell::from(format!("{}", r.primary_key())),
                        if r.all_day() {
                            Cell::from(r.date().format("%m/%d [Day]").to_string())
                        } else {
                            Cell::from(r.datetime().format("%m/%d %H:%M").to_string())
                        },
                        Cell::from(r.detail().to_string()),
                    ]);

                    if (r.all_day() && r.date() == now().date_naive())
                        || r.datetime().date_naive() == now().date_naive()
                    {
                        row = row.underlined()
                    }

                    Some(row)
                } else {
                    None
                }
            })
            .collect::<Vec<Row>>(),
        ListType::Recurring => inner
            .recurring_records
            .iter()
            .map(|r| {
                Row::new(vec![
                    Cell::from(format!("{}", r.recurrence_key())),
                    Cell::from(r.recurrence().to_string()),
                    // going to hell for this
                    Cell::from((&mut r.clone()).record().detail().to_string()),
                ])
            })
            .collect::<Vec<Row>>(),
    };

    let table = Arc::new(
        Table::new(rows.clone())
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(match inner.list_type {
                        ListType::All => "All Events",
                        ListType::Today => "Today's Events",
                        ListType::Recurring => "Recurring Events",
                    }),
            )
            .widths(&[
                Constraint::Length(3),
                Constraint::Percentage(35),
                Constraint::Percentage(65),
            ]),
    );

    if (!rows.is_empty() && inner.events.is_none()) || inner.events.is_some() {
        inner.events = Some((table.clone(), now().naive_local()));
    }

    Ok(table)
}

pub async fn find_dates<'a>(
    state: &mut tokio::sync::MutexGuard<'_, State<'a>>,
    date: chrono::NaiveDateTime,
) -> Vec<Record> {
    let mut v = Vec::new();

    for item in state.records.clone() {
        if item.date() == date.date() {
            v.push(item);
        }
    }

    v
}

pub async fn build_data<'a>(
    state: &mut tokio::sync::MutexGuard<'_, State<'a>>,
    date: chrono::NaiveDateTime,
) -> (Cell<'a>, usize) {
    let mut s = format!("{}\n", date.day());
    for item in find_dates(state, date).await {
        if item.all_day() {
            s += &format!("[Day] {}\n", item.primary_key());
        } else {
            s += &format!(
                "{} {}\n",
                item.datetime().time().format("%H:%M"),
                item.primary_key()
            );
        }
    }

    let style = if date.date() == now().date_naive() {
        *TODAY_STYLE
    } else {
        *CELL_STYLE
    };

    (Cell::from(s.clone()).style(style), s.matches('\n').count())
}

pub fn handle_input(mut buf: String) -> Result<String, anyhow::Error> {
    if event::poll(Duration::from_millis(250))? {
        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char(x) => {
                    buf += &format!("{}", x);
                }
                KeyCode::Enter => {
                    buf += "\n";
                }
                KeyCode::Backspace => {
                    if !buf.is_empty() {
                        buf = buf[0..buf.len() - 1].to_string();
                    }
                }
                _ => {}
            }
        }
    }

    Ok(buf)
}
