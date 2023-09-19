use crate::{
    config::{Config, DBType},
    db::RemoteClient,
    do_client,
    record::{Record, RecordType, RecurringRecord},
    time::now,
};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::Timelike;
use gcal::{
    oauth::{request_access_token, AccessToken},
    resources::{
        CalendarListClient, CalendarListItem, Event, EventCalendarDate, EventClient, EventStatus,
    },
    Client, ClientError,
};
use std::collections::{BTreeMap, BTreeSet};

pub const CALENDAR_SCOPE: &str = "https://www.googleapis.com/auth/calendar";

#[derive(Debug, Clone, Default)]
pub struct GoogleClient {
    client: Option<Client>,
    config: Config,
    ical_map: BTreeMap<String, u64>,
}

impl GoogleClient {
    pub fn new(config: Config) -> Result<Self> {
        if !matches!(config.db_type(), DBType::Google) {
            return Err(anyhow!("DBType must be set to google"));
        }

        if !config.has_client() {
            return Err(anyhow!("Must have client information configured"));
        }

        if config.access_token().is_none() {
            return Err(anyhow!("Must have access token captured"));
        }

        let client = Client::new(
            config.access_token().expect("You must have an access token to make calls. Use `saturn config get-token` to retrieve one."),
        )?;

        Ok(Self {
            client: Some(client),
            config,
            ical_map: Default::default(),
        })
    }

    // this should be safe? lol
    pub fn client(&self) -> Client {
        self.client.clone().unwrap()
    }

    pub fn pick_uid(&self) -> u64 {
        self.ical_map.values().max().cloned().unwrap_or_default() + 1
    }

    pub async fn list_calendars(&mut self) -> Result<Vec<CalendarListItem>, ClientError> {
        let listclient = CalendarListClient::new(self.client().clone());
        do_client!(self, { listclient.list() })
    }

    pub async fn record_to_event(&mut self, calendar_id: String, record: &mut Record) -> Event {
        let start = match record.record_type() {
            RecordType::At => Some(EventCalendarDate {
                date_time: Some(
                    record
                        .datetime()
                        .with_timezone(&chrono_tz::UTC)
                        .to_rfc3339(),
                ),
                time_zone: Some("UTC".to_string()),
                ..Default::default()
            }),
            RecordType::Schedule => {
                let dt = chrono::NaiveDateTime::new(record.date(), record.scheduled().unwrap().0)
                    .and_local_timezone(now().timezone())
                    .unwrap()
                    .with_timezone(&chrono_tz::UTC);
                Some(EventCalendarDate {
                    date_time: Some(dt.to_rfc3339()),
                    time_zone: Some("UTC".to_string()),
                    ..Default::default()
                })
            }
            RecordType::AllDay => Some(EventCalendarDate {
                date_time: Some(
                    chrono::NaiveDateTime::new(
                        record.date(),
                        chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
                    )
                    .and_local_timezone(now().timezone())
                    .unwrap()
                    .with_timezone(&chrono_tz::UTC)
                    .to_rfc3339(),
                ),
                time_zone: Some("UTC".to_string()),
                ..Default::default()
            }),
        };

        let end = match record.record_type() {
            RecordType::At => Some(EventCalendarDate {
                date_time: Some(
                    (record.datetime() + self.config.default_duration().duration())
                        .with_timezone(&chrono_tz::UTC)
                        .to_rfc3339(),
                ),
                time_zone: Some("UTC".to_string()),
                ..Default::default()
            }),
            RecordType::Schedule => {
                let dt = chrono::NaiveDateTime::new(record.date(), record.scheduled().unwrap().1)
                    .and_local_timezone(now().timezone())
                    .unwrap()
                    .with_timezone(&chrono_tz::UTC);

                Some(EventCalendarDate {
                    date_time: Some(dt.to_rfc3339()),
                    time_zone: Some("UTC".to_string()),
                    ..Default::default()
                })
            }
            RecordType::AllDay => Some(EventCalendarDate {
                date_time: Some(
                    (chrono::NaiveDateTime::new(
                        record.date(),
                        chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
                    ) + chrono::Duration::days(1))
                    .and_local_timezone(now().timezone())
                    .unwrap()
                    .with_timezone(&chrono_tz::UTC)
                    .to_rfc3339(),
                ),
                time_zone: Some("UTC".to_string()),
                ..Default::default()
            }),
        };

        let event_client = EventClient::new(self.client());

        let mut f = |record: Record| {
            let mut event = Event::default();
            event.id = record.internal_key();
            event.calendar_id = Some(calendar_id.clone());
            event.ical_uid = if let Some(key) = record.internal_key() {
                if let Some(uid) = self.ical_map.get(&key) {
                    Some(format!("UID:{}", uid))
                } else {
                    let uid = self.pick_uid();
                    self.ical_map.insert(key, uid);
                    Some(format!("UID:{}", uid))
                }
            } else {
                Some(format!("UID:{}", self.pick_uid()))
            };
            event
        };

        let mut event = if let Some(key) = record.internal_key() {
            if let Ok(event) = event_client.get(calendar_id.clone(), key).await {
                event
            } else {
                f(record.clone())
            }
        } else {
            f(record.clone())
        };

        if start.is_some() {
            event.start = start;
        }

        if end.is_some() {
            event.end = end;
        }

        event.calendar_id = Some(calendar_id.clone());
        event.summary = Some(record.detail());

        event
    }

    pub async fn refresh_access_token(&mut self) -> Result<()> {
        let res: Result<AccessToken, ClientError> =
            request_access_token(self.config.clone().into(), None, None, true)
                .await
                .map_err(|e| e.into());
        let token = res?;
        self.config.set_access_token(Some(token.access_token));
        self.config.set_access_token_expires_at(Some(
            now().naive_utc() + chrono::Duration::seconds(token.expires_in),
        ));

        if let Some(refresh_token) = token.refresh_token {
            self.config.set_refresh_token(Some(refresh_token));
            if let Some(expires_in) = token.refresh_token_expires_in {
                self.config.set_refresh_token_expires_at(Some(
                    now().naive_utc() + chrono::Duration::seconds(expires_in),
                ));
            } else {
                self.config.set_refresh_token_expires_at(Some(
                    now().naive_utc() + chrono::Duration::seconds(3600),
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
    ) -> Result<Vec<Record>> {
        let list = EventClient::new(self.client());

        let events = do_client!(self, { list.list(calendar_id.clone(), start, end) })?;

        let mut records = Vec::new();

        for mut event in events {
            if event.recurrence.is_some() {
                event.calendar_id = Some(calendar_id.clone());

                let instances = EventClient::new(self.client())
                    .instances(event.clone())
                    .await?;

                for new_event in instances.items {
                    if let Some(new_start) = &new_event.start {
                        if let Some(new_start) = &new_start.date {
                            if let Ok(new_start) = new_start.parse::<chrono::NaiveDate>() {
                                if new_start > start.date_naive() && new_start < end.date_naive() {
                                    if let Some(status) = new_event.status.clone() {
                                        if !matches!(status, EventStatus::Cancelled) {
                                            records.push(self.event_to_record(new_event)?);
                                        }
                                    }
                                }
                            }
                        } else if let Some(new_start) = &new_start.date_time {
                            if let Ok(new_start) =
                                new_start.parse::<chrono::DateTime<chrono::Local>>()
                            {
                                if new_start > start && new_start < end {
                                    if let Some(status) = new_event.status.clone() {
                                        if !matches!(status, EventStatus::Cancelled) {
                                            records.push(self.event_to_record(new_event)?);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                event.calendar_id = Some(calendar_id.clone());
                if let Some(status) = event.status.clone() {
                    if !matches!(status, EventStatus::Cancelled) {
                        records.push(self.event_to_record(event)?)
                    }
                } else {
                    records.push(self.event_to_record(event)?)
                }
            }
        }

        Ok(records)
    }

    pub fn event_to_record(&mut self, event: Event) -> Result<Record, ClientError> {
        let mut record = Record::default();

        record.set_internal_key(event.id.clone());
        record.set_internal_recurrence_key(event.id.clone());

        let start = event.start;

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
                        x.date.map_or_else(
                            || None,
                            |y| y.parse::<chrono::NaiveDate>().map_or_else(|_| None, Some),
                        )
                    },
                )
            },
            |x| {
                x.date.map_or_else(
                    || None,
                    |y| y.parse::<chrono::NaiveDate>().map_or_else(|_| None, Some),
                )
            },
        );

        let has_start_time = start_time.is_some();

        let has_end_time = match event.end.clone() {
            Some(end) => end.date_time.is_some() || event.end_time_unspecified.unwrap_or_default(),
            None => false,
        };

        let schedule = if has_start_time
            && has_end_time
            && (start_time.unwrap() + chrono::Duration::days(1))
                == event
                    .end
                    .clone()
                    .unwrap()
                    .date_time
                    .unwrap()
                    .parse::<chrono::DateTime<chrono::Local>>()
                    .expect("Couldn't parse time")
                    .naive_local()
        {
            RecordType::AllDay
        } else if has_start_time && has_end_time {
            RecordType::Schedule
        } else if has_start_time {
            RecordType::At
        } else {
            RecordType::AllDay
        };

        record.set_record_type(schedule.clone());

        let now = now();
        let start_time = start_time.unwrap_or(now.naive_local());
        let date = date.unwrap_or(start_time.date());

        match schedule {
            RecordType::AllDay => {
                record.set_all_day();
                record.set_date(date);
            }
            RecordType::At => {
                record.set_at(Some(start_time.time()));
                record.set_date(start_time.date());
            }
            RecordType::Schedule => {
                record.set_date(start_time.date());
                record.set_scheduled(Some((
                    start_time.time(),
                    event.end.map_or_else(
                        || now.time(),
                        |x| {
                            x.date_time.map_or_else(
                                || now.time(),
                                |y| {
                                    y.parse::<chrono::DateTime<chrono::Local>>()
                                        .unwrap_or(now)
                                        .time()
                                },
                            )
                        },
                    ),
                )));
            }
        }

        record.set_detail(event.summary.unwrap_or("No summary provided".to_string()));
        if let Some(uid) = event.ical_uid {
            if let Ok(uid) = uid.strip_prefix("UID:").unwrap_or_default().parse::<u64>() {
                self.ical_map.insert(event.id.unwrap(), uid);
            }
        }
        Ok(record)
    }
}

#[async_trait]
impl RemoteClient for GoogleClient {
    async fn delete(&mut self, calendar_id: String, event_id: String) -> Result<()> {
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
    ) -> Result<Vec<String>> {
        let events = EventClient::new(self.client());
        let mut event = Event::default();
        event.id = Some(event_id);
        event.calendar_id = Some(calendar_id.clone());

        let list = events.instances(event.clone()).await?;

        do_client!(self, { events.delete(event.clone()) })?;

        Ok(list
            .items
            .iter()
            .filter_map(|x| x.id.clone())
            .collect::<Vec<String>>())
    }

    async fn record(&mut self, calendar_id: String, mut record: Record) -> Result<String> {
        let event = self.record_to_event(calendar_id, &mut record).await;
        let client = EventClient::new(self.client());

        let event = do_client!(self, { client.insert(event.clone()) })?;

        if let Some(id) = event.id {
            Ok(id)
        } else {
            Err(anyhow!("Event could not be saved"))
        }
    }

    async fn record_recurrence(
        &mut self,
        calendar_id: String,
        mut record: RecurringRecord,
    ) -> Result<(String, String)> {
        if record.recurrence().duration() < chrono::Duration::days(1) {
            return Err(anyhow!(
                "Google Calendar supports a minimum granularity of 1 day"
            ));
        }

        let mut event = self.record_to_event(calendar_id, record.record()).await;

        let mut recurrence = BTreeSet::default();
        recurrence.insert(record.to_rrule());

        event.recurrence = Some(recurrence);

        let client = EventClient::new(self.client());
        let event = do_client!(self, { client.insert(event.clone()) })?;

        if let Some(id) = event.clone().id {
            return Ok((id.clone(), id));
        }

        Err(anyhow!("Event could not be saved"))
    }

    async fn list_recurrence(&mut self, calendar_id: String) -> Result<Vec<RecurringRecord>> {
        let list = EventClient::new(self.client());

        let mut events = do_client!(self, {
            list.list(
                calendar_id.clone(),
                now() - chrono::Duration::days(30),
                now() + chrono::Duration::days(30),
            )
        })?;

        let mut v = Vec::new();

        for event in &mut events {
            if let Some(recurrence) = &event.recurrence {
                event.calendar_id = Some(calendar_id.clone());
                let record = self.event_to_record(event.clone())?;
                for recur in recurrence {
                    if let Ok(mut x) =
                        RecurringRecord::from_rrule(record.clone(), recur.to_string())
                    {
                        x.set_internal_key(event.id.clone());
                        if let Some(status) = event.status.clone() {
                            if !matches!(status, EventStatus::Cancelled) {
                                v.push(x);
                            }
                        } else {
                            v.push(x);
                        }
                    }
                }
            }
        }

        Ok(v)
    }

    async fn update_recurrence(&mut self, _calendar_id: String) -> Result<()> {
        Ok(())
    }

    async fn list_today(
        &mut self,
        calendar_id: String,
        _include_completed: bool,
    ) -> Result<Vec<Record>> {
        self.perform_list(
            calendar_id,
            now() - chrono::Duration::days(1),
            now() + chrono::Duration::days(1),
        )
        .await
    }

    async fn list_all(
        &mut self,
        calendar_id: String,
        _include_completed: bool, // FIXME include tasks
    ) -> Result<Vec<Record>> {
        self.perform_list(
            calendar_id,
            (now()
                .with_hour(0)
                .unwrap()
                .with_minute(0)
                .unwrap()
                .with_second(0)
                .unwrap())
                - chrono::Duration::days(30),
            (now() + chrono::Duration::days(30))
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
    ) -> Result<Vec<Record>> {
        self.perform_list(calendar_id, now() - last, now()).await
    }

    async fn complete_task(&mut self, _calendar_id: String, _primary_key: u64) -> Result<()> {
        Ok(())
    }

    async fn get(&mut self, calendar_id: String, event_id: String) -> Result<Record> {
        let events = EventClient::new(self.client());
        Ok(self.event_to_record(events.get(calendar_id, event_id).await?)?)
    }

    async fn get_recurring(
        &mut self,
        calendar_id: String,
        event_id: String,
    ) -> Result<RecurringRecord> {
        let events = EventClient::new(self.client());
        let event = events.get(calendar_id, event_id).await?;
        let mut ret: Option<RecurringRecord> = None;

        let record = self.event_to_record(event.clone())?;
        for recur in &event
            .recurrence
            .ok_or(anyhow!("No recurrence data for this event"))?
        {
            if let Ok(rr) = RecurringRecord::from_rrule(record.clone(), recur.clone()) {
                ret = Some(rr);
                break;
            }
        }

        let mut ret = ret.ok_or(anyhow!("No recurrence data found for event"))?;
        ret.set_internal_key(event.id.clone());
        Ok(ret)
    }

    async fn update(&mut self, calendar_id: String, mut record: Record) -> Result<()> {
        let events = EventClient::new(self.client());
        let event = self.record_to_event(calendar_id, &mut record).await;
        events.update(event).await?;
        Ok(())
    }

    async fn update_recurring(
        &mut self,
        calendar_id: String,
        mut record: RecurringRecord,
    ) -> Result<()> {
        let events = EventClient::new(self.client());
        let key = record.internal_key();
        let r = record.record();
        r.set_internal_key(key);
        let mut event = self.record_to_event(calendar_id, r).await;
        event.recurrence = Some(BTreeSet::from_iter(vec![record.to_rrule()]));
        events.update(event).await?;
        Ok(())
    }
}
