use davisjr::prelude::*;
use google_calendar::Client;
use std::sync::Arc;
use tokio::sync::Mutex;

pub type State = Arc<Mutex<ClientParameters>>;

#[derive(Clone, Debug, Default)]
pub struct ClientParameters {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_url: Option<String>,
    pub access_key: Option<String>,
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
    let mut client = Client::new(
        lock.client_id.clone(),
        lock.client_secret.clone(),
        lock.redirect_url.clone().unwrap(),
        "",
        "",
    );

    let token = client
        .get_access_token(&code.unwrap(), &oauth_state.unwrap())
        .await?;

    eprintln!("{:?}", token);
    lock.access_key = Some(token.access_token);

    Ok((
        req,
        Some(Response::new(Body::from(
            "Please close this browser tab. Thanks!".to_string(),
        ))),
        state,
    ))
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
