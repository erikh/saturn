use chrono::Timelike;

pub fn now() -> chrono::DateTime<chrono::Local> {
    chrono::Local::now()
}

pub fn window() -> (
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
}
