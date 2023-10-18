use chrono::Timelike;

lazy_static::lazy_static! {
    pub static ref UPDATE_INTERVAL: chrono::Duration = chrono::Duration::minutes(5);
}

pub fn now() -> chrono::DateTime<chrono::Local> {
    chrono::Local::now()
}

pub fn window(
    config: &crate::config::Config,
) -> (
    chrono::DateTime<chrono::Local>,
    chrono::DateTime<chrono::Local>,
) {
    (
        (now()
            .with_hour(0)
            .unwrap()
            .with_minute(0)
            .unwrap()
            .with_second(0)
            .unwrap()
            - config.query_window()),
        (now() + config.query_window())
            .with_hour(0)
            .unwrap()
            .with_minute(0)
            .unwrap()
            .with_second(0)
            .unwrap(),
    )
}
