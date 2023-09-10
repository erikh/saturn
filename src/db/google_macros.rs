#[macro_export]
macro_rules! do_client {
    ($obj:ident, $block:block) => {{
        use $crate::time::now;

        'end: loop {
            if let Some(expires) = $obj.config.access_token_expires_at() {
                if expires - chrono::Duration::hours(1) < now().naive_utc() {
                    $obj.refresh_access_token().await?;
                }
            }

            match $block.await {
                Ok(t) => {
                    break 'end Ok(t);
                }
                Err(e) => match e {
                    ClientError::InvalidToken => {
                        $obj.refresh_access_token().await?;
                    }
                    _ => break 'end Err(e),
                },
            }
        }
    }};
}
