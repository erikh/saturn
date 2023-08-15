#[macro_export]
macro_rules! do_client {
    ($obj:ident, $block:block) => {
        'end: loop {
            match $block.await {
                Ok(t) => {
                    break 'end Ok(t);
                }
                Err(e) => match e {
                    ClientError::InvalidToken => {
                        let res: Result<AccessToken, ClientError> =
                            request_access_token($obj.config.clone().into(), None, None, true)
                                .await
                                .map_err(|e| e.into());
                        let token = res?;
                        $obj.config.set_access_token(Some(token.access_token));
                        $obj.config.set_access_token_expires_at(Some(
                            chrono::Local::now().naive_utc()
                                + chrono::Duration::seconds(token.expires_in),
                        ));

                        if let Some(refresh_token) = token.refresh_token {
                            $obj.config.set_refresh_token(Some(refresh_token));
                            if let Some(expires_in) = token.refresh_token_expires_in {
                                $obj.config.set_refresh_token_expires_at(Some(
                                    chrono::Local::now().naive_utc()
                                        + chrono::Duration::seconds(expires_in),
                                ));
                            } else {
                                $obj.config.set_refresh_token_expires_at(Some(
                                    chrono::Local::now().naive_utc()
                                        + chrono::Duration::seconds(3600),
                                ));
                            }
                        }

                        $obj.config.save(None)?;
                    }
                    _ => break 'end Err(e),
                },
            }
        }
    };
}
