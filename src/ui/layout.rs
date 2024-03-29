use crate::{
    record::{PresentedRecord, PresentedRecurringRecord, Record, RecurringRecord},
    time::now,
    ui::{
        consts::*,
        state::{ProtectedState, State},
        types::*,
    },
};
use anyhow::{anyhow, Result};
use chrono::Datelike;
use crossterm::event::{self, Event, KeyCode};
use ratatui::{prelude::*, widgets::*};
use std::time::Duration;
use std::{io::Stdout, ops::Deref, sync::Arc};

fn sit<T>(msg: impl std::future::Future<Output = Result<T>>) -> Result<T> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    runtime.block_on(msg)
}

pub async fn draw_loop<'a>(
    state: ProtectedState<'static>,
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
) -> Result<()> {
    let (s, mut r) = tokio::sync::mpsc::channel(1);

    let s2 = state.clone();
    std::thread::spawn(move || sit(read_input(s2, s)));
    let mut last_line = String::from("placeholder");
    let mut last_draw = now() - chrono::TimeDelta::try_minutes(1).unwrap_or_default();

    loop {
        let mut lock = state.lock().await;
        if !lock.block_ui {
            let redraw = lock.redraw;

            if redraw {
                lock.redraw = false;
            }

            if !lock.errors.is_empty() {
                lock.redraw = true;
            }

            let line = lock.line_buf.clone();
            drop(lock);

            if redraw
                || line != last_line
                || last_draw + chrono::TimeDelta::try_seconds(5).unwrap_or_default() < now()
            {
                let lock = state.lock().await;
                let show = lock.show.clone();
                let show_recurring = lock.show_recurring.clone();
                drop(lock);
                terminal.draw(|f| {
                    render_app(state.clone(), f, line.clone(), show, show_recurring);
                })?;

                last_line = line;
                last_draw = now();
            }

            if r.try_recv().is_ok() {
                break;
            }
        }
        tokio::time::sleep(Duration::new(0, 100)).await;
    }
    Ok(())
}

fn notify_update_state(state: ProtectedState<'static>) {
    tokio::spawn(async move {
        state.add_notification("Updating state").await;
        state.update_state().await
    });
}

pub async fn read_input<'a>(
    state: ProtectedState<'static>,
    s: tokio::sync::mpsc::Sender<()>,
) -> Result<()> {
    let mut last_buf = String::new();

    'input: loop {
        let lock = state.lock().await;
        if !lock.block_ui {
            let mut buf = lock.line_buf.clone();
            drop(lock);

            buf = match handle_input(buf) {
                Ok(buf) => buf,
                Err(_) => {
                    state.add_error(anyhow!("Invalid Input")).await;
                    state.update_state().await;
                    continue 'input;
                }
            };

            let mut lock = state.lock().await;
            if buf != last_buf && !lock.errors.is_empty() {
                lock.errors = Vec::new();
                if !buf.is_empty() {
                    buf = buf[0..buf.len() - 1].to_string();
                }
            }
            drop(lock);

            if buf.ends_with('\n') {
                match buf.trim() {
                    "quit" => break 'input,
                    x => {
                        if x.starts_with("s ") || x.starts_with("show ") {
                            let m = if x.starts_with("show ") {
                                x.trim_start_matches("show ")
                            } else {
                                x.trim_start_matches("s ")
                            }
                            .trim()
                            .split(' ')
                            .filter(|x| !x.is_empty())
                            .collect::<Vec<&str>>();
                            let mut lock = state.lock().await;
                            lock.show = None;
                            lock.show_recurring = None;
                            drop(lock);
                            match m[0] {
                                "all" | "a" => {
                                    state.lock().await.list_type = ListType::All;
                                    notify_update_state(state.clone());
                                }
                                "today" | "t" => {
                                    state.lock().await.list_type = ListType::Today;
                                    let state = state.clone();
                                    notify_update_state(state.clone());
                                }
                                "recur" | "recurring" | "recurrence" | "r" => {
                                    if m.len() == 2 {
                                        if let Ok(id) = m[1].parse::<u64>() {
                                            state
                                                .lock()
                                                .await
                                                .commands
                                                .push(CommandType::Show(true, id));
                                        } else {
                                            state
                                                .add_error(anyhow!("Invalid Command '{}'", x))
                                                .await
                                        }
                                    } else {
                                        state.lock().await.list_type = ListType::Recurring;
                                    }

                                    notify_update_state(state.clone());
                                }
                                id => {
                                    if let Ok(id) = id.parse::<u64>() {
                                        state
                                            .lock()
                                            .await
                                            .commands
                                            .push(CommandType::Show(false, id));
                                    } else {
                                        state.add_error(anyhow!("Invalid Command '{}'", x)).await
                                    }

                                    notify_update_state(state.clone());
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
                            let mut recur = false;

                            for id in &ids {
                                if id.is_empty() {
                                    continue;
                                }

                                if *id == "recur" {
                                    recur = true;
                                    continue;
                                }

                                match id.parse::<u64>() {
                                    Ok(y) => v.push(y),
                                    Err(_) => {
                                        state.add_error(anyhow!("Invalid ID {}", id)).await;
                                    }
                                };
                            }

                            let command = if recur {
                                CommandType::DeleteRecurring(v)
                            } else {
                                CommandType::Delete(v)
                            };

                            let s = state.clone();
                            tokio::spawn(async move {
                                s.lock().await.commands.push(command);
                            });

                            notify_update_state(state.clone());
                        } else if x.starts_with("e ") || x.starts_with("entry ") {
                            let x = x.to_string();

                            let state = state.clone();
                            tokio::spawn(async move {
                                state.lock().await.commands.push(CommandType::Entry(
                                    if x.starts_with("entry ") {
                                        x.trim_start_matches("entry ")
                                    } else {
                                        x.trim_start_matches("e ")
                                    }
                                    .to_string(),
                                ));
                                notify_update_state(state.clone());
                            });
                        } else if x.starts_with("edit ") {
                            let ids = x
                                .trim_start_matches("edit ")
                                .split(' ')
                                .filter(|x| !x.is_empty())
                                .collect::<Vec<&str>>();

                            let mut v = Vec::new();
                            let mut recur = false;

                            'ids: for id in &ids {
                                if id.is_empty() {
                                    continue;
                                }

                                if *id == "recur" {
                                    recur = true;
                                    continue;
                                }

                                match id.parse::<u64>() {
                                    Ok(y) => {
                                        // we only need the first one
                                        v.push(y);
                                        break 'ids;
                                    }
                                    Err(_) => {
                                        state.add_error(anyhow!("Invalid ID {}", id)).await;
                                    }
                                };
                            }

                            let s = state.clone();
                            tokio::spawn(async move {
                                if v.is_empty() {
                                    s.add_error(anyhow!("Edit requires an ID")).await;
                                } else {
                                    s.lock().await.commands.push(CommandType::Edit(recur, v[0]));
                                }
                            });

                            notify_update_state(state.clone());
                        } else if x.starts_with("/ ") || x.starts_with("search") {
                            let x = x.to_string();

                            let state = state.clone();
                            tokio::spawn(async move {
                                state.lock().await.commands.push(CommandType::Search(
                                    if x.starts_with("search ") {
                                        x.trim_start_matches("search ")
                                    } else {
                                        x.trim_start_matches("/ ")
                                    }
                                    .to_string()
                                    .split(" ")
                                    .filter_map(|x| {
                                        if x.is_empty() {
                                            None
                                        } else {
                                            Some(x.to_string())
                                        }
                                    })
                                    .collect(),
                                ));
                                notify_update_state(state.clone());
                            });
                        } else {
                            state.add_error(anyhow!("Invalid Command")).await;
                        }
                    }
                }
                buf = String::new();
            }
            last_buf = buf.clone();
            state.lock().await.line_buf = buf;
            tokio::time::sleep(Duration::new(0, 500000)).await;
        } else {
            tokio::time::sleep(Duration::new(1, 0)).await;
        }
    }
    s.send(()).await?;
    Ok(())
}

// blatantly taken from ratatui examples
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ]
            .as_ref(),
        )
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ]
            .as_ref(),
        )
        .split(popup_layout[1])[1]
}

pub fn add_error(state: ProtectedState<'static>, e: anyhow::Error) {
    // I apparently hate myself
    let _ = std::thread::spawn(move || {
        sit(async move {
            state.lock().await.errors.push(e.to_string());
            Ok(())
        })
    })
    .join();
}

pub fn get_errors(state: ProtectedState<'static>) -> Option<Vec<String>> {
    std::thread::spawn(move || {
        sit(async move {
            let errors = state.lock().await.errors.clone();
            if errors.is_empty() {
                Ok(None)
            } else {
                Ok(Some(errors))
            }
        })
    })
    .join()
    .unwrap()
    .unwrap()
}

pub fn render_error(
    frame: &mut ratatui::Frame<'_, CrosstermBackend<Stdout>>,
    layout: Rect,
    e: String,
) {
    let layout = centered_rect(50, 20, layout);
    let block = Block::default()
        .title("Error")
        .title_style(Style::default().fg(Color::Red))
        .borders(Borders::ALL);
    let area = block.inner(layout);

    let paragraph = Paragraph::new(e + "\nPress any key to continue\n")
        .style(Style::default().fg(Color::LightRed))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    frame.render_widget(Clear, layout);
    frame.render_widget(block, layout);
    frame.render_widget(paragraph, area);
}

pub fn render_app(
    state: ProtectedState<'static>,
    frame: &mut ratatui::Frame<'_, CrosstermBackend<Stdout>>,
    buf: String,
    show: Option<Record>,
    show_recurring: Option<RecurringRecord>,
) {
    // NOTE: I apologize for making you read this code

    let layout = Layout::default()
        .constraints([Constraint::Length(1), Constraint::Percentage(100)].as_ref())
        .split(frame.size());

    let line_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(30), Constraint::Length(30)].as_ref())
        .split(layout[0]);

    let draw_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)].as_ref())
        .split(layout[1]);

    let s = state.clone();
    let res = std::thread::spawn(move || sit(build_events(s))).join();

    if let Ok(Ok(events)) = res {
        let s = state.clone();
        let res = std::thread::spawn(move || {
            sit(async move {
                let mut lock = s.lock().await;
                let ret = lock.notification.clone();

                if let Some(ret) = &ret {
                    if now().naive_local()
                        >= ret.1 + chrono::TimeDelta::try_seconds(1).unwrap_or_default()
                    {
                        lock.notification = None;
                    }
                }

                Ok(ret)
            })
        })
        .join();

        if let Ok(Ok(notification)) = res {
            if let Some(notification) = notification {
                frame.render_widget(
                    Paragraph::new(format!("[ {} ]", notification.0)).alignment(Alignment::Right),
                    line_layout[1],
                );
            }

            if let Some(record) = show {
                let s = state.clone();
                let res = std::thread::spawn(move || sit(build_show_event(s, record))).join();
                if let Ok(Ok(event)) = res {
                    frame.render_widget(event.deref().clone(), draw_layout[0]);
                } else if let Ok(Err(e)) = res {
                    add_error(state.clone(), e);
                } else {
                    add_error(
                        state.clone(),
                        anyhow!("Unknown error while showing an event"),
                    );
                }
            } else if let Some(record) = show_recurring {
                let s = state.clone();
                let res =
                    std::thread::spawn(move || sit(build_show_recurring_event(s, record))).join();
                if let Ok(Ok(event)) = res {
                    frame.render_widget(event.deref().clone(), draw_layout[0]);
                } else if let Ok(Err(e)) = res {
                    add_error(state.clone(), e);
                } else {
                    add_error(
                        state.clone(),
                        anyhow!("Unknown error while showing an event"),
                    );
                }
            } else {
                let s = state.clone();
                let res = std::thread::spawn(move || sit(build_calendar(s))).join();
                if let Ok(Ok(calendar)) = res {
                    frame.render_widget(calendar.deref().clone(), draw_layout[0]);
                } else if let Ok(Err(e)) = res {
                    add_error(state.clone(), e);
                } else {
                    add_error(
                        state.clone(),
                        anyhow!("Unknown error while showing calendar"),
                    );
                }
            }

            frame.render_widget(events.deref().clone(), draw_layout[1]);
        } else if let Ok(Err(e)) = res {
            add_error(state.clone(), e);
        } else {
            add_error(
                state.clone(),
                anyhow!("Unknown error while polling for notifications"),
            );
        }
    } else if let Ok(Err(e)) = res {
        add_error(state.clone(), e);
    } else {
        add_error(state.clone(), anyhow!("Unknown error while listing events"));
    }

    if let Some(errors) = get_errors(state.clone()) {
        render_error(frame, layout[1], errors.join("\n").to_string())
    }

    frame.render_widget(Paragraph::new(format!(">> {}", buf)), layout[0]);
    frame.set_cursor(3 + buf.len() as u16, 0);
}

async fn get_month_name(state: ProtectedState<'static>) -> &str {
    match chrono::Month::try_from(now().month() as u8) {
        Ok(m) => m.name(),
        Err(_) => {
            state.add_error(anyhow!("Invalid Month")).await;
            notify_update_state(state.clone());
            ""
        }
    }
}

pub async fn build_show_recurring_event<'a>(
    state: ProtectedState<'static>,
    record: RecurringRecord,
) -> Result<Arc<Table<'a>>> {
    let header_cells = ["Key", "Value"]
        .iter()
        .map(|h| Cell::from(*h).style(*TITLE_STYLE));
    let header = Row::new(header_cells)
        .style(*HEADER_STYLE)
        .height(1)
        .bottom_margin(1);

    let presented: PresentedRecurringRecord = record.clone().into();
    let mut rows = vec![
        Row::new(vec![
            Cell::from("id"),
            Cell::from(format!("{}", record.recurrence_key())),
        ]),
        Row::new(vec![
            Cell::from("date"),
            Cell::from(format!("{}", presented.record.date)),
        ]),
        Row::new(vec![
            Cell::from("recurrence"),
            Cell::from(format!("{}", presented.recurrence.to_string())),
        ]),
        Row::new(vec![
            Cell::from("completed"),
            Cell::from(format!("{}", presented.record.completed)),
        ]),
        Row::new(vec![
            Cell::from("detail"),
            Cell::from(format!("{}", presented.record.detail)),
        ]),
        Row::new(vec![
            Cell::from("type"),
            Cell::from(format!("{:?}", presented.record.typ)),
        ]),
    ];

    match presented.record.typ {
        crate::record::RecordType::At => rows.push(Row::new(vec![
            Cell::from("at"),
            Cell::from(format!("{}", presented.record.at.unwrap().format("%H:%M"))),
        ])),
        crate::record::RecordType::Schedule => rows.push(Row::new(vec![
            Cell::from("scheduled"),
            Cell::from(format!("{}", presented.record.scheduled.unwrap())),
        ])),
        _ => {}
    }

    let table = Arc::new(
        Table::new(rows.clone())
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(get_month_name(state).await),
            )
            .widths(&[Constraint::Percentage(30), Constraint::Percentage(70)]),
    );
    Ok(table)
}

pub async fn build_show_event<'a>(
    state: ProtectedState<'static>,
    record: Record,
) -> Result<Arc<Table<'a>>> {
    let header_cells = ["Key", "Value"]
        .iter()
        .map(|h| Cell::from(*h).style(*TITLE_STYLE));
    let header = Row::new(header_cells)
        .style(*HEADER_STYLE)
        .height(1)
        .bottom_margin(1);

    let presented: PresentedRecord = record.clone().into();
    let mut rows = vec![
        Row::new(vec![
            Cell::from("id"),
            Cell::from(format!("{}", record.primary_key())),
        ]),
        Row::new(vec![
            Cell::from("date"),
            Cell::from(format!("{}", presented.date)),
        ]),
        Row::new(vec![
            Cell::from("completed"),
            Cell::from(format!("{}", presented.completed)),
        ]),
        Row::new(vec![
            Cell::from("detail"),
            Cell::from(format!("{}", presented.detail)),
        ]),
        Row::new(vec![
            Cell::from("type"),
            Cell::from(format!("{:?}", presented.typ)),
        ]),
        Row::new(vec![
            Cell::from("fields"),
            Cell::from(record.fields().to_string()),
        ]),
    ];

    match presented.typ {
        crate::record::RecordType::At => rows.push(Row::new(vec![
            Cell::from("at"),
            Cell::from(format!("{}", presented.at.unwrap().format("%H:%M"))),
        ])),
        crate::record::RecordType::Schedule => rows.push(Row::new(vec![
            Cell::from("scheduled"),
            Cell::from(format!("{}", presented.scheduled.unwrap())),
        ])),
        _ => {}
    }

    let table = Arc::new(
        Table::new(rows.clone())
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(get_month_name(state).await),
            )
            .widths(&[Constraint::Percentage(30), Constraint::Percentage(70)]),
    );
    Ok(table)
}
pub async fn build_calendar<'a>(state: ProtectedState<'static>) -> Result<Arc<Table<'a>>> {
    if let Some(calendar) = state.lock().await.calendar.clone() {
        if calendar.1 + chrono::TimeDelta::try_seconds(1).unwrap_or_default() > now().naive_local()
        {
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
            (date
                - chrono::TimeDelta::try_days(datetime.weekday().num_days_from_sunday().into())
                    .unwrap_or_default())
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
        begin += chrono::TimeDelta::try_days(1).unwrap_or_default();
    }
    drop(lock);
    last_row.push((Cell::from("".to_string()), 0));
    rows.push(
        Row::new(last_row.iter().map(|x| x.0.clone()).collect::<Vec<Cell>>()).height({
            let res = last_row.iter().map(|x| x.1).max().unwrap_or(4) as u16;
            if res > 4 {
                res
            } else {
                4
            }
        }),
    );

    let table = Arc::new(
        Table::new(rows.clone())
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(get_month_name(state.clone()).await),
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

pub async fn build_events<'a>(state: ProtectedState<'static>) -> Result<Arc<Table<'a>>> {
    if let Some(events) = state.lock().await.events.clone() {
        if events.1 + chrono::TimeDelta::try_seconds(1).unwrap_or_default() > now().naive_local() {
            return Ok(events.0);
        }
    }

    let datetime = now();
    let date = datetime.date_naive();
    let begin = chrono::NaiveDateTime::new(
        chrono::NaiveDate::from_ymd_opt(
            date.year_ce().1 as i32,
            date.month0() + 1,
            (date
                - chrono::TimeDelta::try_days(datetime.weekday().num_days_from_sunday().into())
                    .unwrap_or_default())
            .day0()
                + 1,
        )
        .unwrap(),
        chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
    );

    let header_cells = ["ID", "Time", "Summary"]
        .iter()
        .map(|h| Cell::from(*h).style(*TITLE_STYLE));
    let header = Row::new(header_cells)
        .style(*HEADER_STYLE)
        .height(1)
        .bottom_margin(1);

    let mut inner = state.lock().await;
    let rows = match inner.list_type {
        ListType::All | ListType::Today | ListType::Search => inner
            .records
            .iter()
            .filter_map(|r| {
                if (r.all_day()
                    && chrono::NaiveDateTime::new(
                        r.date(),
                        chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
                    ) >= begin)
                    || r.datetime().naive_local() >= begin
                {
                    let pk = format!("{}", r.primary_key());
                    let detail = r.detail().to_string();

                    let mut row = Row::new(vec![
                        Cell::from(pk),
                        if r.all_day() {
                            Cell::from(r.date().format("%m/%d [Day]").to_string())
                        } else {
                            Cell::from(r.datetime().format("%m/%d %H:%M").to_string())
                        },
                        Cell::from(detail),
                    ])
                    .style(Style::default().fg(Color::DarkGray));

                    if r.datetime().date_naive() == now().date_naive() {
                        row = row.style(Style::default().fg(Color::White))
                    }

                    if (r.all_day() && r.date() == now().date_naive())
                        || (datetime
                            > r.datetime() - chrono::TimeDelta::try_hours(1).unwrap_or_default()
                            && datetime
                                < r.datetime()
                                    + chrono::TimeDelta::try_hours(1).unwrap_or_default())
                    {
                        row = row.style(Style::default().fg(Color::LightGreen))
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
                let pk = format!("{}", r.recurrence_key());
                let detail = r.clone().record().detail().to_string();
                Row::new(vec![
                    Cell::from(pk),
                    Cell::from(r.recurrence().to_string()),
                    Cell::from(detail),
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
                        ListType::Search => "Search Results",
                    }),
            )
            .widths(&[
                Constraint::Length(5),
                Constraint::Length(15),
                Constraint::Percentage(100),
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

pub fn handle_input(mut buf: String) -> Result<String> {
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
