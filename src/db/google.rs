#![allow(dead_code)]
use crate::{
    config::{Config, DBType},
    db::memory::MemoryDB,
};
use anyhow::anyhow;
use google_calendar::{
    calendar_list::CalendarList,
    events::Events,
    types::{MinAccessRole, OrderBy},
    Client,
};

pub const CALENDAR_SCOPE: &str = "https://www.googleapis.com/auth/calendar";

pub struct GoogleLoader {
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

        eprintln!("here");
        let calendars = client.list_all(MinAccessRole::Owner, false, false).await?;

        eprintln!("here");
        if calendars.status != 200 {
            return Err(anyhow!(
                "Google Calendar produced a non-200 response to requesting calendars"
            ));
        }

        eprintln!("here");
        let client = Events {
            client: self.client.clone(),
        };

        eprintln!("here");
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

        eprintln!("here");
        if events.status != 200 {
            return Err(anyhow!(
                "Google Calendar produced a non-200 response to requesting events"
            ));
        }

        eprintln!("here");
        for event in events.body {
            println!("{}", event.summary);
        }

        Ok(MemoryDB::new())
    }

    pub async fn dump(&self, _db: &mut Box<MemoryDB>) -> Result<(), anyhow::Error> {
        Ok(())
    }
}
