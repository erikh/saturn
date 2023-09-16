use crate::config::Config;
use anyhow::{anyhow, Result};
use gcal::{oauth_listener, oauth_user_url, ClientParameters, State};
use tokio::sync::Mutex;

pub async fn get_access_token() -> Result<()> {
    let mut config = Config::load(None)?;

    if !config.has_client() {
        return Err(anyhow!(
            "You need to configure a client first; see `saturn config set-client`"
        ));
    }

    let mut params = ClientParameters {
        client_id: config.client_id().unwrap(),
        client_secret: config.client_secret().unwrap(),
        ..Default::default()
    };

    let state = State::new(Mutex::new(params.clone()));
    let host = oauth_listener(state.clone()).await?;
    params.redirect_url = Some(format!("http://{}", host));

    let url = oauth_user_url(params.clone());
    println!("Click on this and login: {}", url);

    loop {
        let lock = state.lock().await;
        if lock.access_key.is_some() {
            config.set_access_token(lock.access_key.clone());
            config.set_access_token_expires_at(lock.expires_at);
            config.set_refresh_token(lock.refresh_token.clone());
            config.set_refresh_token_expires_at(lock.refresh_token_expires_at);
            config.set_redirect_url(params.redirect_url.clone());
            config.save(None)?;
            println!("Captured. Thanks!");
            return Ok(());
        }

        tokio::time::sleep(std::time::Duration::new(1, 0)).await;
    }
}
