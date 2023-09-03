use crate::{
    config::{Config, DBType},
    db::RemoteClient,
    do_client,
    record::{Record, RecordType, RecurringRecord},
};
use anyhow::anyhow;
use async_trait::async_trait;
use chrono::Timelike;
use gcal::{
    oauth::{request_access_token, AccessToken},
    resources::{
        CalendarListClient, CalendarListItem, Event, EventCalendarDate, EventClient, EventStatus,
    },
    Client, ClientError,
};

pub const CALENDAR_SCOPE: &str = "https://www.googleapis.com/auth/calendar";

#[derive(Debug, Clone, Default)]
pub struct GoogleClient {
    client: Option<Client>,
    config: Config,
}

pub fn record_to_event(calendar_id: String, record: Record) -> Event {
    let start = match record.record_type() {
        RecordType::At => Some(EventCalendarDate {
            date_time: Some(
                record
                    .datetime()
                    .with_timezone(&chrono_tz::UTC)
                    .to_rfc3339(),
            ),
            ..Default::default()
        }),
        RecordType::Schedule => {
            let dt = chrono::NaiveDateTime::new(record.date(), record.scheduled().unwrap().0)
                .and_local_timezone(chrono::Local::now().timezone())
                .unwrap()
                .with_timezone(&chrono_tz::UTC);
            Some(EventCalendarDate {
                date_time: Some(dt.to_rfc3339()),
                ..Default::default()
            })
        }
        RecordType::AllDay => {
            let date = record
                .datetime()
                .with_timezone(&chrono_tz::UTC)
                .date_naive();
            Some(EventCalendarDate {
                date: Some(date.format("%Y-%m-%d").to_string()),
                ..Default::default()
            })
        }
    };

    let end = match record.record_type() {
        RecordType::At => Some(EventCalendarDate {
            date_time: Some(
                (record.datetime() + chrono::Duration::minutes(15))
                    .with_timezone(&chrono_tz::UTC)
                    .to_rfc3339(),
            ),
            ..Default::default()
        }),
        RecordType::Schedule => {
            let dt = chrono::NaiveDateTime::new(record.date(), record.scheduled().unwrap().1)
                .and_local_timezone(chrono::Local::now().timezone())
                .unwrap()
                .with_timezone(&chrono_tz::UTC);

            Some(EventCalendarDate {
                date_time: Some(dt.to_rfc3339()),
                ..Default::default()
            })
        }
        RecordType::AllDay => start.clone(),
    };

    let mut event = Event::default();
    event.calendar_id = Some(calendar_id);
    event.ical_uid = Some(format!("UID:{}", record.primary_key()));
    if start.is_some() {
        event.start = start;
    }

    if end.is_some() {
        event.end = end;
    }

    event.summary = Some(record.detail());

    event
}

impl GoogleClient {
    pub fn new(config: Config) -> Result<Self, anyhow::Error> {
        if !matches!(config.db_type(), DBType::Google) {
            return Err(anyhow!("DBType must be set to google").into());
        }

        if !config.has_client() {
            return Err(anyhow!("Must have client information configured").into());
        }

        if config.access_token().is_none() {
            return Err(anyhow!("Must have access token captured").into());
        }

        let client = Client::new(
            config.access_token().expect("You must have an access token to make calls. Use `saturn config get-token` to retreive one."),
        )?;

        Ok(Self {
            client: Some(client),
            config,
        })
    }

    // this should be safe? lol
    pub fn client(&self) -> Client {
        self.client.clone().unwrap()
    }

    pub async fn list_calendars(&mut self) -> Result<Vec<CalendarListItem>, ClientError> {
        let listclient = CalendarListClient::new(self.client().clone());
        do_client!(self, { listclient.list() })
    }

    pub async fn refresh_access_token(&mut self) -> Result<(), anyhow::Error> {
        let res: Result<AccessToken, ClientError> =
            request_access_token(self.config.clone().into(), None, None, true)
                .await
                .map_err(|e| e.into());
        let token = res?;
        self.config.set_access_token(Some(token.access_token));
        self.config.set_access_token_expires_at(Some(
            chrono::Local::now().naive_utc() + chrono::Duration::seconds(token.expires_in),
        ));

        if let Some(refresh_token) = token.refresh_token {
            self.config.set_refresh_token(Some(refresh_token));
            if let Some(expires_in) = token.refresh_token_expires_in {
                self.config.set_refresh_token_expires_at(Some(
                    chrono::Local::now().naive_utc() + chrono::Duration::seconds(expires_in),
                ));
            } else {
                self.config.set_refresh_token_expires_at(Some(
                    chrono::Local::now().naive_utc() + chrono::Duration::seconds(3600),
                ));
            }
        }

        self.config.save(None)?;
        Ok(())
    }

    async fn perform_list(
        &mut self,
        calendar_id: String,
        start: chrono::DateTime<chrono::Local>,
        end: chrono::DateTime<chrono::Local>,
    ) -> Result<Vec<Record>, anyhow::Error> {
        let list = EventClient::new(self.client());

        let events = do_client!(self, { list.list(calendar_id.clone(), start, end) })?;

        let mut records = Vec::new();

        for mut event in events {
            if event.recurrence.is_some() {
                let mut changed = false;
                event.calendar_id = Some(calendar_id.clone());

                let instances = EventClient::new(self.client())
                    .instances(event.clone())
                    .await?;

                if !instances.items.is_empty() {
                    let new_event = &mut instances.items.last().unwrap().clone();
                    if let Some(new_start) = &new_event.start {
                        if let Some(new_start) = &new_start.date_time {
                            if let Ok(new_start) =
                                new_start.parse::<chrono::DateTime<chrono::Local>>()
                            {
                                if new_start > start && new_start < end {
                                    new_event.calendar_id = event.calendar_id;
                                    event = new_event.clone();
                                    changed = true;
                                }
                            }
                        }
                    }
                }

                if !changed {
                    continue;
                }
            }

            event.calendar_id = Some(calendar_id.clone());
            if let Some(status) = event.status.clone() {
                if !matches!(status, EventStatus::Cancelled) {
                    records.push(self.event_to_record(event).await?)
                }
            } else {
                records.push(self.event_to_record(event).await?)
            }
        }

        Ok(records)
    }

    pub async fn event_to_record(&self, event: Event) -> Result<Record, ClientError> {
        let mut record = Record::default();

        record.set_internal_key(event.id.clone());
        record.set_internal_recurrence_key(event.recurring_event_id.clone());

        let start = event.start.clone();

        let start_time = event.original_start_time.clone().map_or_else(
            || {
                start.clone().map_or_else(
                    || None,
                    |x| {
                        x.date_time.map_or_else(
                            || None,
                            |y| {
                                y.parse::<chrono::DateTime<chrono::Local>>()
                                    .map_or_else(|_| None, |z| Some(z.naive_local()))
                            },
                        )
                    },
                )
            },
            |x| {
                x.date_time.map_or_else(
                    || None,
                    |y| {
                        y.parse::<chrono::DateTime<chrono::Local>>()
                            .map_or_else(|_| None, |z| Some(z.naive_local()))
                    },
                )
            },
        );

        let date = event.original_start_time.clone().map_or_else(
            || {
                start.map_or_else(
                    || None,
                    |x| {
                        x.date_time.map_or_else(
                            || None,
                            |y| {
                                y.parse::<chrono::DateTime<chrono::Local>>()
                                    .map_or_else(|_| None, |z| Some(z.date_naive()))
                            },
                        )
                    },
                )
            },
            |x| {
                x.date_time.map_or_else(
                    || None,
                    |y| {
                        y.parse::<chrono::DateTime<chrono::Local>>()
                            .map_or_else(|_| None, |z| Some(z.date_naive()))
                    },
                )
            },
        );

        let has_start_time = start_time.is_some();

        let has_end_time = match event.end.clone() {
            Some(end) => end.date_time.is_some() || event.end_time_unspecified.unwrap_or_default(),
            None => false,
        };

        let schedule = if !has_start_time && !has_end_time {
            RecordType::AllDay
        } else if has_start_time && has_end_time {
            RecordType::Schedule
        } else if has_start_time {
            RecordType::At
        } else {
            RecordType::AllDay
        };

        record.set_record_type(schedule.clone());

        let now = chrono::Local::now();
        let date = date.unwrap_or(now.date_naive());
        let start_time = start_time.unwrap_or(now.naive_local());

        match schedule {
            RecordType::AllDay => {
                record.set_all_day(true);
                record.set_date(date);
            }
            RecordType::At => {
                record.set_at(Some(start_time.time()));
                record.set_date(date);
            }
            RecordType::Schedule => {
                record.set_date(date);
                record.set_scheduled(Some((
                    start_time.time(),
                    event.end.map_or_else(
                        || chrono::Local::now().time(),
                        |x| {
                            x.date_time.map_or_else(
                                || chrono::Local::now().time(),
                                |y| {
                                    y.parse::<chrono::DateTime<chrono::Local>>()
                                        .unwrap_or(chrono::Local::now())
                                        .time()
                                },
                            )
                        },
                    ),
                )));
            }
        }

        record.set_detail(event.summary.unwrap_or("No summary provided".to_string()));
        Ok(record)
    }
}

#[async_trait]
impl RemoteClient for GoogleClient {
    async fn delete(&mut self, calendar_id: String, event_id: String) -> Result<(), anyhow::Error> {
        let events = EventClient::new(self.client());
        let mut event = Event::default();
        event.id = Some(event_id);
        event.calendar_id = Some(calendar_id);

        do_client!(self, { events.delete(event.clone()) })?;
        Ok(())
    }

    async fn delete_recurrence(
        &mut self,
        calendar_id: String,
        event_id: String,
    ) -> Result<(), anyhow::Error> {
        self.delete(calendar_id, event_id).await?;
        Ok(())
    }

    async fn record(
        &mut self,
        calendar_id: String,
        record: Record,
    ) -> Result<String, anyhow::Error> {
        let event = record_to_event(calendar_id, record);
        let client = EventClient::new(self.client());

        let event = do_client!(self, { client.insert(event.clone()) })?;

        if let Some(id) = event.id {
            Ok(id)
        } else {
            Err(anyhow!("Event could not be saved").into())
        }
    }

    async fn record_recurrence(
        &mut self,
        _calendar_id: String,
        _record: RecurringRecord,
    ) -> Result<String, anyhow::Error> {
        Ok(String::new())
    }

    async fn list_recurrence(
        &mut self,
        _calendar_id: String,
    ) -> Result<Vec<RecurringRecord>, anyhow::Error> {
        Ok(Vec::new())
    }

    async fn update_recurrence(&mut self, _calendar_id: String) -> Result<(), anyhow::Error> {
        Ok(())
    }

    async fn list_today(
        &mut self,
        calendar_id: String,
        _include_completed: bool,
    ) -> Result<Vec<Record>, anyhow::Error> {
        let now = chrono::Local::now();

        self.perform_list(calendar_id, now, now + chrono::Duration::days(1))
            .await
    }

    async fn list_all(
        &mut self,
        calendar_id: String,
        _include_completed: bool, // FIXME include tasks
    ) -> Result<Vec<Record>, anyhow::Error> {
        let now = chrono::Local::now();

        self.perform_list(
            calendar_id,
            (now.with_hour(0)
                .unwrap()
                .with_minute(0)
                .unwrap()
                .with_second(0)
                .unwrap())
                - chrono::Duration::days(7),
            (now + chrono::Duration::days(1))
                .with_hour(0)
                .unwrap()
                .with_minute(0)
                .unwrap()
                .with_second(0)
                .unwrap(),
        )
        .await
    }

    async fn events_now(
        &mut self,
        calendar_id: String,
        last: chrono::Duration,
        _include_completed: bool,
    ) -> Result<Vec<Record>, anyhow::Error> {
        let now = chrono::Local::now();
        self.perform_list(calendar_id, now - last, now).await
    }

    async fn complete_task(
        &mut self,
        _calendar_id: String,
        _primary_key: u64,
    ) -> Result<(), anyhow::Error> {
        Ok(())
    }
}
