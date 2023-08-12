use crate::{
    config::{Config, DBType},
    db::{memory::MemoryDB, RemoteClient},
    record::{Record, RecurringRecord},
};
use anyhow::anyhow;
use google_calendar::{
    calendar_list::CalendarList,
    events::Events,
    types::{MinAccessRole, OrderBy, SendUpdates},
    Client,
};

pub const CALENDAR_SCOPE: &str = "https://www.googleapis.com/auth/calendar";

pub struct GoogleLoader {
    client: Client,
}

pub struct GoogleClient {
    #[allow(dead_code)]
    client: Client,
}

impl GoogleLoader {
    pub fn new(config: Config) -> Result<Self, anyhow::Error> {
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
            config
                .client_id()
                .expect("Client ID was not stored. Use `saturn config set-client` to store this."),
            config.client_secret().expect(
                "Client Secret was not stored. Use `saturn config set-client` to store this.",
            ),
            config.redirect_url().expect("Expected a redirect_url to be populated as a part of the `saturn config get-token`"),
            config.access_token().expect("You must have an access token to make calls. Use `saturn config get-token` to retreive one."),
            "",
        );

        Ok(Self { client })
    }

    pub async fn load(&self) -> Result<Box<MemoryDB>, anyhow::Error> {
        let client = CalendarList {
            client: self.client.clone(),
        };

        let calendars = client.list_all(MinAccessRole::Owner, false, false).await?;

        if calendars.status != 200 {
            return Err(anyhow!(
                "Google Calendar produced a non-200 response to requesting calendars"
            ));
        }

        let client = Events {
            client: self.client.clone(),
        };

        let events = client
            .list_all(
                &calendars.body[0].id,
                "",
                0,
                OrderBy::StartTime,
                &[],
                "",
                &[],
                false,
                false,
                true,
                &(chrono::Local::now() + chrono::Duration::days(1)).to_rfc3339(),
                &(chrono::Local::now() - chrono::Duration::days(7)).to_rfc3339(),
                &chrono::Local::now().offset().to_string(),
                "",
            )
            .await?;

        if events.status != 200 {
            return Err(anyhow!(
                "Google Calendar produced a non-200 response to requesting events"
            ));
        }

        for event in events.body {
            println!(
                "{} {} {}",
                event.id,
                event.start.map_or("All Day".to_string(), |s| s
                    .date_time
                    .unwrap()
                    .with_timezone(&chrono::Local)
                    .to_string()),
                event.summary
            );
        }

        Ok(MemoryDB::new())
    }

    pub async fn dump(&self, _db: &mut Box<MemoryDB>) -> Result<(), anyhow::Error> {
        Ok(())
    }
}

impl GoogleClient {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait::async_trait]
impl RemoteClient for GoogleClient {
    async fn delete(&self, calendar_id: String, event_id: String) -> Result<(), anyhow::Error> {
        let events = Events {
            client: self.client.clone(),
        };

        match events
            .delete(&calendar_id, &event_id, false, SendUpdates::All)
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow!(e)),
        }
    }

    async fn delete_recurrence(
        &self,
        calendar_id: String,
        event_id: String,
    ) -> Result<(), anyhow::Error> {
        self.delete(calendar_id, event_id).await
    }

    async fn record(&self, _calendar_id: String, _record: Record) -> Result<(), anyhow::Error> {
        Ok(())
    }

    async fn record_recurrence(
        &self,
        _calendar_id: String,
        _record: RecurringRecord,
    ) -> Result<(), anyhow::Error> {
        Ok(())
    }

    async fn list_recurrence(
        &self,
        _calendar_id: String,
    ) -> Result<Vec<RecurringRecord>, anyhow::Error> {
        Ok(Vec::new())
    }

    async fn update_recurrence(&self, _calendar_id: String) -> Result<(), anyhow::Error> {
        Ok(())
    }

    async fn list_today(
        &self,
        _calendar_id: String,
        _include_completed: bool,
    ) -> Result<Vec<Record>, anyhow::Error> {
        Ok(Vec::new())
    }

    async fn list_all(
        &self,
        _calendar_id: String,
        _include_completed: bool,
    ) -> Result<Vec<Record>, anyhow::Error> {
        Ok(Vec::new())
    }

    async fn events_now(
        &self,
        _calendar_id: String,
        _last: chrono::Duration,
        _include_completed: bool,
    ) -> Result<Vec<Record>, anyhow::Error> {
        Ok(Vec::new())
    }

    async fn complete_task(
        &self,
        _calendar_id: String,
        _primary_key: u64,
    ) -> Result<(), anyhow::Error> {
        Ok(())
    }
}
