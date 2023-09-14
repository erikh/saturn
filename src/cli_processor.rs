// this is a very filthy macro. be careful when modifying it.
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
            Command::Delete { ids, recur } => {
                for id in ids {
                    if recur {
                        $db.delete_recurrence(id).await?;
                    } else {
                        $db.delete(id).await?;
                    }
                }
            }
            Command::Notify {
                well,
                timeout,
                include_completed,
                icon,
            } => {
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
                        let mut n = notification.body(&format_at(entry, at));
                        if let Some(icon) = icon.clone() {
                            n = n.icon(&icon);
                        }

                        n.show()?;
                    } else if let Some(schedule) = entry.scheduled() {
                        let start = chrono::NaiveDateTime::new(
                            $crate::time::now().date_naive(),
                            schedule.0,
                        );
                        let lower = start - get_well(well.clone())?;
                        let upper = start + get_well(well.clone())?;
                        let local = $crate::time::now().naive_local();

                        if lower < local && local < upper {
                            let mut n = notification.body(&format_scheduled(entry, schedule));
                            if let Some(icon) = icon.clone() {
                                n = n.icon(&icon);
                            }

                            n.show()?;
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
                    let mut list = if all {
                        $db.list_all(false).await?
                    } else {
                        $db.list_today(false).await?
                    };
                    list.sort_by($crate::cli::sort_records);
                    print_entries(list);
                }
            }
            Command::Today {} => {
                print_entries($db.list_today(false).await?);
            }
            Command::Entry { args } => {
                $db.record_entry($crate::entry::EntryParser::new(args))
                    .await?;
            }
            Command::Edit { recur, id } => {
                if recur {
                    let record = $db.get_recurring(id).await?;
                    let presented: $crate::record::PresentedRecurringRecord = record.clone().into();
                    let f = tempfile::NamedTempFile::new()?;
                    serde_yaml::to_writer(&f, &presented)?;
                    let (f, path) = f.keep()?;
                    drop(f);
                    let mut cmd = tokio::process::Command::new(
                        std::env::var("EDITOR")
                            .expect("You must have $EDITOR set to use this command"),
                    );
                    cmd.args([path.clone()]);
                    let mut child = cmd.spawn()?;
                    if child.wait().await?.success() {
                        let mut io = std::fs::OpenOptions::new();
                        io.read(true);
                        let f = io.open(path)?;
                        let presented: $crate::record::PresentedRecurringRecord =
                            serde_yaml::from_reader(&f)?;
                        $db.update_recurring(presented.to_record(
                            record.clone().record().primary_key(),
                            record.recurrence_key(),
                            record.clone().record().internal_key(),
                            record.internal_key(),
                        ))
                        .await?;
                    }
                } else {
                    let record = $db.get(id).await?;
                    let presented: $crate::record::PresentedRecord = record.clone().into();
                    let f = tempfile::NamedTempFile::new()?;
                    serde_yaml::to_writer(&f, &presented)?;
                    let (f, path) = f.keep()?;
                    drop(f);
                    let mut cmd = tokio::process::Command::new(
                        std::env::var("EDITOR")
                            .expect("You must have $EDITOR set to use this command"),
                    );
                    cmd.args([path.clone()]);
                    let mut child = cmd.spawn()?;
                    if child.wait().await?.success() {
                        let mut io = std::fs::OpenOptions::new();
                        io.read(true);
                        let f = io.open(path)?;
                        let presented: $crate::record::PresentedRecord =
                            serde_yaml::from_reader(&f)?;
                        $db.update(presented.to_record(
                            record.primary_key(),
                            record.recurrence_key(),
                            record.internal_key(),
                            record.internal_recurrence_key(),
                        ))
                        .await?;
                    }
                }
            }
            Command::Dump { recur, id } => {
                if recur {
                    let presented: $crate::record::PresentedRecurringRecord =
                        $db.get_recurring(id).await?.into();
                    println!("{}", serde_yaml::to_string(&presented)?);
                } else {
                    let presented: $crate::record::PresentedRecord = $db.get(id).await?.into();
                    println!("{}", serde_yaml::to_string(&presented)?);
                }
            }
        }

        $db.dump().await?;
    };
}

#[macro_export]
macro_rules! list_ui {
    ($db:ident, $list_type:ident) => {{
        $db.load().await?;
        let all = match $list_type {
            $crate::ui::types::ListType::All => $db.list_all(true).await?,
            $crate::ui::types::ListType::Today => $db.list_today(true).await?,
            $crate::ui::types::ListType::Recurring => Vec::new(),
        };

        $db.dump().await?;

        Ok(all)
    }};
}

#[macro_export]
macro_rules! process_ui_command {
    ($db:ident, $command:ident) => {{
        if $command.is_some() {
            $db.load().await?;
            match $command.unwrap() {
                $crate::ui::types::CommandType::Delete(items) => {
                    for item in items {
                        $db.delete(item).await?
                    }
                }
                $crate::ui::types::CommandType::DeleteRecurring(items) => {
                    for item in items {
                        $db.delete_recurrence(item).await?;
                    }
                }
                $crate::ui::types::CommandType::Entry(entry) => {
                    let parts = entry
                        .split(' ')
                        .filter(|x| !x.is_empty())
                        .map(|s| s.to_string())
                        .collect::<Vec<String>>();
                    $db.record_entry($crate::entry::EntryParser::new(parts))
                        .await?;
                }
            };
            $db.dump().await?;
        }
    }};
}
