use davisjr::prelude::*;
use reqwest::{header::HeaderMap, ClientBuilder};
use serde_derive::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::db::google::CALENDAR_SCOPE;

const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const USER_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";

pub type State = Arc<Mutex<ClientParameters>>;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AccessToken {
    pub token_type: Option<String>,
    pub access_token: String,
    pub expires_in: i64,
    pub refresh_token: Option<String>,
    pub refresh_token_expires_in: Option<i64>,
    pub scope: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct ClientParameters {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_url: Option<String>,
    pub access_key: Option<String>,
    pub expires_at: Option<chrono::NaiveDateTime>,
    pub refresh_token: Option<String>,
    pub refresh_token_expires_at: Option<chrono::NaiveDateTime>,
}

impl From<crate::config::Config> for ClientParameters {
    fn from(value: crate::config::Config) -> Self {
        Self {
            client_id: value.client_id().unwrap_or_default(),
            client_secret: value.client_secret().unwrap_or_default(),
            redirect_url: value.redirect_url(),
            access_key: value.access_token(),
            expires_at: value.access_token_expires_at(),
            refresh_token: value.refresh_token(),
            refresh_token_expires_at: value.refresh_token_expires_at(),
        }
    }
}

async fn handler(
    req: Request<Body>,
    _resp: Option<Response<Body>>,
    _params: Params,
    app: App<State, NoState>,
    state: NoState,
) -> HTTPResult<NoState> {
    let pairs = req
        .uri()
        .query()
        .map(|s| {
            s.split("&")
                .map(|n| n.split("=").collect::<Vec<&str>>())
                .collect::<Vec<Vec<&str>>>()
        })
        .unwrap_or(Vec::new());

    let mut code: Option<String> = None;
    let mut oauth_state: Option<String> = None;

    for pair in pairs {
        if pair[0] == "code" {
            code = Some(pair[1].to_string());
        } else if pair[0] == "state" {
            oauth_state = Some(pair[1].to_string());
        }

        if code.is_some() && oauth_state.is_some() {
            break;
        }
    }

    let lock = app.state().await.unwrap();
    let lock = lock.lock().await;
    let mut lock = lock.lock().await;

    let token =
        request_access_token(lock.clone(), code.as_deref(), oauth_state.as_deref(), false).await?;
    lock.access_key = Some(token.access_token);
    lock.expires_at =
        Some(chrono::Local::now().naive_utc() + chrono::Duration::seconds(token.expires_in));

    if let Some(refresh_token) = token.refresh_token {
        lock.refresh_token = Some(refresh_token);
        if let Some(expires_in) = token.refresh_token_expires_in {
            lock.refresh_token_expires_at =
                Some(chrono::Local::now().naive_utc() + chrono::Duration::seconds(expires_in));
        } else {
            lock.refresh_token_expires_at =
                Some(chrono::Local::now().naive_utc() + chrono::Duration::seconds(3600));
        }
    }

    Ok((
        req,
        Some(Response::new(Body::from(
            "Please close this browser tab. Thanks!".to_string(),
        ))),
        state,
    ))
}

pub async fn request_access_token(
    client_params: ClientParameters,
    code: Option<&str>,
    state: Option<&str>,
    refresh: bool,
) -> Result<AccessToken, Error> {
    let grant = if refresh {
        "refresh_token"
    } else {
        "authorization_code"
    };

    let mut params = vec![
        ("grant_type", grant),
        ("client_id", &client_params.client_id),
        ("client_secret", &client_params.client_secret),
    ];

    let mut headers = HeaderMap::default();
    headers.insert(
        reqwest::header::ACCEPT,
        reqwest::header::HeaderValue::from_static("application/json"),
    );

    let redirect_url = client_params.redirect_url.unwrap_or_default();
    let token = if refresh {
        client_params.refresh_token.unwrap()
    } else {
        Default::default()
    };

    if !refresh {
        params.push(("code", code.unwrap()));
        params.push(("redirect_uri", &redirect_url));
        params.push(("state", state.unwrap()));
    } else {
        params.push(("refresh_token", &token));
    }

    let client = ClientBuilder::new()
        .default_headers(headers)
        .https_only(true)
        .build()?;

    Ok(client
        .post(TOKEN_URL)
        .form(&params)
        .basic_auth(&client_params.client_id, Some(&client_params.client_secret))
        .send()
        .await?
        .json()
        .await?)
}

pub fn oauth_user_url(params: ClientParameters) -> String {
    // using the uuid is taken from a sight read of google_calendar; I'm not
    // sure it's necessary to use a uuid but I am lazy
    let u = uuid::Uuid::new_v4();
    format!(
        "{}?client_id={}&access_type=offline&response_type=code&redirect_uri={}&state={}&scope={}",
        USER_URL,
        params.client_id,
        params.redirect_url.expect("Expected a redirect URL"),
        u,
        CALENDAR_SCOPE,
    )
}

pub async fn oauth_listener(state: State) -> Result<String, ServerError> {
    let mut app = App::with_state(state.clone());

    app.get("/", compose_handler!(handler))?;

    // find a free port. this is susceptible to timing races and if that happens I guess they'll
    // just have to start the program again.
    let lis = tokio::net::TcpListener::bind("localhost:0").await?;
    let addr = lis.local_addr()?.clone();
    drop(lis);

    let mut lock = state.lock().await;
    lock.redirect_url = Some(format!("http://{}", addr.to_string()));

    tokio::spawn(async move { app.serve(&addr.to_string()).await.unwrap() });

    Ok(addr.to_string())
}
