#[macro_export]
macro_rules! process_cli {
    ($cli:ident, $config:ident, $db:ident) => {
        use saturn_cli::db::google::GoogleClient;

        process_cli!($cli, $config, $db, None::<GoogleClient>);
    };
    ($cli:ident, $config:ident, $db:ident, $client:expr) => {
        $db.load().await?;

        match $cli.command {
            Command::Config { command } => match command {
                ConfigCommand::SetClient {
                    client_id,
                    client_secret,
                } => set_client_info(client_id, client_secret)?,
                ConfigCommand::GetToken {} => get_access_token().await?,
                ConfigCommand::SetSyncWindow { window } => {
                    set_sync_window(FancyDuration::<chrono::Duration>::parse(&window)?)?
                }
                ConfigCommand::DBType { db_type } => set_db_type(db_type)?,
                ConfigCommand::ListCalendars => {
                    if $client.is_none() {
                        eprintln!("Not supported in unixfile mode");
                    } else {
                        list_calendars($client.unwrap()).await?;
                    }
                }
                ConfigCommand::SetCalendarID { id } => {
                    if $client.is_none() {
                        eprintln!("Not supported in unixfile mode");
                    } else {
                        set_calendar_id(id, $config)?;
                    }
                }
            },
            Command::Complete { id } => $db.complete_task(id).await?,
            Command::Delete { id, recur } => {
                if recur {
                    $db.delete_recurrence(id).await?
                } else {
                    $db.delete(id).await?
                }
            }
            Command::Notify {
                well,
                timeout,
                include_completed,
            } => {
                let now = chrono::Local::now();

                let timeout = timeout.map_or(std::time::Duration::new(60, 0), |t| {
                    fancy_duration::FancyDuration::<std::time::Duration>::parse(&t)
                        .expect("Invalid Duration")
                        .duration()
                });

                let mut notification = notify_rust::Notification::new();
                notification.summary("Calendar Event");
                notification.timeout(timeout);

                for entry in $db
                    .events_now(get_well(well.clone())?, include_completed)
                    .await?
                {
                    if let Some(at) = entry.at() {
                        notification.body(&format_at(entry, at)).show()?;
                    } else if let Some(schedule) = entry.scheduled() {
                        let start = chrono::NaiveDateTime::new(now.date_naive(), schedule.0);
                        let lower = start - get_well(well.clone())?;
                        let upper = start + get_well(well.clone())?;
                        let local = now.naive_local();

                        if lower < local && local < upper {
                            notification
                                .body(&format_scheduled(entry, schedule))
                                .show()?;
                        }
                    }
                }
            }
            Command::Now {
                well,
                include_completed,
            } => {
                print_entries($db.events_now(get_well(well)?, include_completed).await?);
            }
            Command::List { all, recur } => {
                if recur {
                    print_recurring($db.list_recurrence().await?);
                } else {
                    if all {
                        print_entries($db.list_all(false).await?);
                    } else {
                        print_entries($db.list_today(false).await?);
                    }
                }
            }
            Command::Today {} => {
                print_entries($db.list_today(false).await?);
            }
            Command::Entry { args } => {
                $db.record_entry(EntryParser::new(args)).await?;
            }
        }

        $db.dump().await?;
    };
}
