#[macro_export]
macro_rules! launch_editor {
    ($db: ident, $id: ident, $typ:ty, $fetch:ident, $recur: ident) => {{
        let record = $db.$fetch($id).await?;
        let presented: $typ = record.clone().into();
        let f = tempfile::NamedTempFile::new()?;
        serde_yaml::to_writer(&f, &presented)?;
        let (f, path) = f.keep()?;
        drop(f);
        let mut cmd = tokio::process::Command::new(
            std::env::var("EDITOR").unwrap_or("/usr/bin/vim".to_string()),
        );
        cmd.args([path.clone()]);
        let mut child = cmd.spawn()?;
        if child.wait().await?.success() {
            let mut io = std::fs::OpenOptions::new();
            io.read(true);
            let f = io.open(path)?;
            let presented: $typ = serde_yaml::from_reader(&f)?;
            $crate::update_record!($db, presented, record, $recur);
        }
    }};
}

#[macro_export]
macro_rules! map_record {
    ($db: ident, $id:ident, true) => {{
        $db.get_recurring($id).await
    }};
    ($db: ident, $id:ident, false) => {{
        $db.get($id).await
    }};
    ($db: ident, $id: ident) => {{
        map_record!($db, $id, false)
    }};
}

#[macro_export]
macro_rules! update_record {
    ($db: ident, $presented: ident, $record:ident, true) => {{
        $db.update_recurring($presented.to_record(
            $record.clone().record().primary_key(),
            $record.recurrence_key(),
            $record.clone().record().internal_key(),
            $record.internal_key(),
        ))
        .await?;
    }};
    ($db: ident, $presented: ident, $record:ident, false) => {{
        $db.update($presented.to_record(
            $record.primary_key(),
            $record.recurrence_key(),
            $record.internal_key(),
            $record.internal_recurrence_key(),
        ))
        .await?;
    }};
}

// this is a very filthy macro. be careful when modifying it.
#[macro_export]
macro_rules! process_cli {
    ($cli:ident, $config:ident, $db:ident) => {
        use $crate::db::google::GoogleClient;

        process_cli!($cli, $config, $db, None::<GoogleClient>);
    };
    ($cli:ident, $config:ident, $db:ident, $client:expr) => {
        $db.load().await?;

        match $cli.command {
            Command::Config { command } => match command {
                ConfigCommand::SetQueryWindow { set } => {
                    let mut config = Config::load(None)?;
                    config.set_query_window(FancyDuration::parse(&set)?.duration());
                    config.save(None)?;
                }
                ConfigCommand::Set24hTime { set } => {
                    let mut config = Config::load(None)?;
                    config.set_use_24h_time(set);
                    config.save(None)?;
                }
                ConfigCommand::SetClient {
                    client_id,
                    client_secret,
                } => {
                    let mut config = Config::load(None)?;
                    config.set_client_info(client_id, client_secret);
                    config.save(None)?;
                }
                ConfigCommand::GetToken {} => $crate::oauth::get_access_token().await?,
                ConfigCommand::DBType { db_type } => {
                    let mut config = Config::load(None)?;
                    let typ = match db_type.as_str() {
                        "google" => DBType::Google,
                        "unixfile" => DBType::UnixFile,
                        _ => {
                            return Err(anyhow!(
                                "Invalid db type: valid types are `google` and `unixfile`"
                            ))
                        }
                    };

                    config.set_db_type(typ);
                    config.save(None)?;
                }
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
                ConfigCommand::SetDefaultDuration { duration } => {
                    let duration: fancy_duration::FancyDuration<chrono::Duration> =
                        fancy_duration::FancyDuration::parse(&duration)?;
                    let mut config = $crate::config::Config::load(None)?;
                    config.set_default_duration(Some(duration));
                    config.save(None)?;
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
                    list.sort_by($crate::record::sort_records);
                    print_entries(list);
                }
            }
            Command::Today {} => {
                print_entries($db.list_today(false).await?);
            }
            Command::Entry { args } => {
                $db.list_all(false).await?;
                $db.record_entry($crate::parsers::entry::EntryParser::new(
                    args,
                    $config.use_24h_time(),
                ))
                .await?;
            }
            Command::Edit { recur, id } => {
                if recur {
                    $crate::launch_editor!(
                        $db,
                        id,
                        $crate::record::PresentedRecurringRecord,
                        get_recurring,
                        true
                    );
                } else {
                    $crate::launch_editor!($db, id, $crate::record::PresentedRecord, get, false);
                }
            }
            Command::Show { recur, id } => {
                if recur {
                    let presented: $crate::record::PresentedRecurringRecord =
                        $db.get_recurring(id).await?.into();
                    println!("{}", serde_yaml::to_string(&presented)?);
                } else {
                    let presented: $crate::record::PresentedRecord = $db.get(id).await?.into();
                    println!("{}", serde_yaml::to_string(&presented)?);
                }
            }
            Command::Search { terms } => {
                let parser =
                    $crate::parsers::search::SearchParser::new(terms, $db.list_all(false).await?);
                print_entries(parser.perform()?);
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
            $crate::ui::types::ListType::Recurring | $crate::ui::types::ListType::Search => {
                Vec::new()
            }
        };

        $db.dump().await?;

        Ok(all)
    }};
}

#[macro_export]
macro_rules! process_ui_command {
    ($obj:ident, $db:ident, $config:ident) => {{
        let mut lock = $obj.lock().await;
        let commands = lock.commands.clone();
        lock.commands = Vec::new();
        lock.block_ui = true;
        drop(lock);
        $db.load().await?;
        for command in commands {
            match command {
                $crate::ui::types::CommandType::Search(terms) => {
                    let parser = $crate::parsers::search::SearchParser::new(
                        terms,
                        $db.list_all(false).await?,
                    );
                    let mut inner = $obj.lock().await;
                    inner.list_type = $crate::ui::types::ListType::Search;
                    inner.records = parser.perform()?;
                    inner.records.sort_by($crate::record::sort_records);
                    inner.redraw = true;
                }
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
                    $db.list_all(false).await?;
                    let parts = entry
                        .split(' ')
                        .filter(|x| !x.is_empty())
                        .map(|s| s.to_string())
                        .collect::<Vec<String>>();
                    $db.record_entry($crate::parsers::entry::EntryParser::new(
                        parts,
                        $config.use_24h_time(),
                    ))
                    .await?;
                }
                $crate::ui::types::CommandType::Edit(recur, id) => {
                    if recur {
                        $crate::launch_editor!(
                            $db,
                            id,
                            $crate::record::PresentedRecurringRecord,
                            get_recurring,
                            true
                        );
                    } else {
                        $crate::launch_editor!(
                            $db,
                            id,
                            $crate::record::PresentedRecord,
                            get,
                            false
                        );
                    }
                }
                $crate::ui::types::CommandType::Show(recur, id) => {
                    if recur {
                        let mut lock = $obj.lock().await;
                        lock.show_recurring = Some($db.get_recurring(id).await?);
                        drop(lock);
                    } else {
                        let mut lock = $obj.lock().await;
                        lock.show = Some($db.get(id).await?);
                        drop(lock);
                    }
                }
            };
        }
        $db.dump().await?;
        let mut lock = $obj.lock().await;
        lock.block_ui = false;
    }};
}
