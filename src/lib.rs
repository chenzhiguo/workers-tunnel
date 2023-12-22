use crate::proxy::{parse_early_data, run_tunnel};
use crate::websocket::WebSocketConnection;
use worker::*;

mod proxy;
mod websocket;
mod types;

#[event(fetch)]
async fn main(req: Request, env: Env, _: Context) -> Result<Response> {
    console_log!(
        "{} {}, located at: {:?}, within: {}",
        req.method().to_string(),
        req.path(),
        req.cf().coordinates().unwrap_or_default(),
        req.cf().region().unwrap_or("unknown region".into())
    );
    if let Some(webstr) = req.headers().get("Upgrade")? {
        // if webstr == "websocket" {
        //     console_log!("header is websocket!");
        // } else {
        //     console_log!("header is not websocket!");
        // }
        // Response::ok("Hello, World! ".to_owned() + &*webstr)
        // get user id
        let user_id = env.var("USER_ID")?.to_string();

        // ready early data
        let early_data = req.headers().get("sec-websocket-protocol")?;
        let early_data = parse_early_data(early_data)?;
        console_log!("Get early_data is {:?}", early_data);

        // Accept / handle a websocket connection
        let pair = WebSocketPair::new()?;
        let server = pair.server;
        server.accept()?;
        console_log!("Websocket server accept success!");

        wasm_bindgen_futures::spawn_local(async move {
            let event_stream = server.events().expect("could not open stream");

            let socket = WebSocketConnection::new(&server, event_stream, early_data);

            // run vless tunnel
            match run_tunnel(socket, &user_id).await {
                Err(err) => {
                    if err.kind() == std::io::ErrorKind::InvalidData || err.kind() == std::io::ErrorKind::ConnectionAborted
                    {
                        server.close(Some(1003), Some("Unsupported data"))
                            .unwrap_or_default()
                    }
                }
                _ => (),
            }
        });

        Response::from_websocket(pair.client)
    } else {
        Router::new()
            .get_async("/foo", handle_get)
            .run(req, env)
            .await
    }

}

pub async fn handle_get(_: Request, _ctx: RouteContext<()>) -> Result<Response> {
    Response::from_json(&types::GenericResponse {
        status: 200,
        message: "You reached a GET route!".to_string(),
    })
}