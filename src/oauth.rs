use davisjr::prelude::*;
use std::sync::Arc;
use tokio::sync::Mutex;

pub type State = Arc<Mutex<Option<String>>>;

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

    for pair in pairs {
        if pair[0] == "state" {
            app.state()
                .await
                .unwrap()
                .lock()
                .await
                .lock()
                .await
                .replace(pair[1].to_string());
            break;
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

pub async fn oauth_listener(state: State) -> Result<String, ServerError> {
    let mut app = App::with_state(state);

    app.get("/", compose_handler!(handler))?;

    // find a free port. this is susceptible to timing races and if that happens I guess they'll
    // just have to start the program again.
    let lis = tokio::net::TcpListener::bind("localhost:0").await?;
    let addr = lis.local_addr()?.clone();
    drop(lis);

    tokio::spawn(async move { app.serve(&addr.to_string()).await.unwrap() });

    Ok(addr.to_string())
}
