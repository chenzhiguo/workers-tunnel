use reqwest::Client;
use serde_json::{from_str, Value};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request as Req, RequestInit, RequestMode, Response as Resp};
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
    // get user id
    let user_id = env.var("USER_ID")?.to_string();
    if let Some(webstr) = req.headers().get("Upgrade")? {
        // if webstr == "websocket" {
        //     console_log!("header is websocket!");
        // } else {
        //     console_log!("header is not websocket!");
        // }
        // Response::ok("Hello, World! ".to_owned() + &*webstr)


        let mut proxy_ip = types::PROXY_IPS[rand::random::<usize>() % crate::types::PROXY_IPS.len()];
        unsafe { types::PROXY_IP = proxy_ip; }

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
            .get_async("/cf", handle_get_cf)
            .get_async(&format!("/{}", user_id), handle_get_index)
            .get_async(&format!("/bestip/{}", user_id), handle_get_best)
            .get_async("/", handle_get_fake_index)
            .run(req, env)
            .await
    }
}

pub async fn handle_get_cf(mut request: Request, _ctx: RouteContext<()>) -> Result<Response> {
    let mut response = Response::from_json(&types::GenericResponse {
        status: 200,
        message: request.text().await?,
    });
    return response;
}

pub async fn handle_get_index(mut request: Request, _ctx: RouteContext<()>) -> Result<Response> {
    let fake_host = types::FAKE_HOSTS[rand::random::<usize>() % crate::types::FAKE_HOSTS.len()];
    console_log!("request url: {}", fake_host);
    let client = Client::new();
    // let response = client.get("http://httpbin.org/get").send().await.unwrap();
    let response = client.get(format!("https://{}{}",fake_host, request.url().unwrap().path())).send().await.unwrap();
    // let mut response = Response::from_bytes(response.bytes().await.expect("No response!").to_vec());
    let mut headers = Headers::new();
    headers.set("content-type", "text/html")?;

    let data = response.bytes().await.expect("No response!").to_vec();
    let mut response = Response::from_body(ResponseBody::Body(data));
    // response.unwrap().with_headers(headers);
    return response;
}

pub async fn handle_get_fake_index(mut request: Request, _ctx: RouteContext<()>) -> Result<Response> {
    let fake_host = types::FAKE_HOSTS[rand::random::<usize>() % crate::types::FAKE_HOSTS.len()];
    console_log!("request url: {}", fake_host);
    let client = Client::new();
    // let response = client.get("http://httpbin.org/get").send().await.unwrap();
    let response = client.get(format!("https://{}{}",fake_host,request.url().unwrap().path())).send().await.unwrap();
    // let mut response = Response::from_bytes(response.bytes().await.expect("No response!").to_vec());
    let mut headers = Headers::new();
    headers.set("content-type", "text/html")?;

    let data = response.bytes().await.expect("No response!").to_vec();
    let mut response = Response::from_body(ResponseBody::Body(data));
    // response.unwrap().with_headers(headers);
    return response;
}

pub async fn handle_get_best(mut request: Request, ctx: RouteContext<()>) -> Result<Response> {
    let best_ip_link = format!("https://sub.xijingping.gay/auto?host={}&uuid={}",
                               request.headers().get("Host")?.ok_or_else(|| "None")?,
                               ctx.env.var("USER_ID")?.to_string());
    let client = Client::new();
    console_log!("request url: {}", best_ip_link);
    // let response = client.get("http://httpbin.org/get").send().await.unwrap();
    let response = client.get(best_ip_link).send().await.unwrap();

    // let json: Value = from_str(&response.text().await.unwrap()).unwrap();
    // let json = &response.text().await.unwrap();
    // console_log!("{}", json);
    // let resp = reqwest::get(best_ip_link)
    //     .await?
    //     .json::<String>()
    //     .await?;
    // println!("{:#?}", resp);
    // let resp = fetch_url(best_ip_link).await?;
    // let mut res: Resp = resp.dyn_into().unwrap();
    // 获取 JSON 数据
    // let json = res.as_string().ok_or_else(|| "{}")?;
    // Response::from_json(&types::GenericResponse {
    //     status: 200,
    //     message: json.to_string(),
    // })
    let data = response.bytes().await.expect("No response!").to_vec();
    Response::from_body(ResponseBody::Body(data))
}

#[wasm_bindgen]
#[allow(dead_code)]
pub async fn fetch_url(repo: String) -> std::result::Result<JsValue, JsValue> {
    let mut opts = RequestInit::new();
    opts.method("GET");
    opts.mode(RequestMode::SameOrigin);

    let url = format!("{}", repo);

    let request = Req::new_with_str_and_init(&url, &opts)?;

    request
        .headers()
        .set("Accept", "application/json")?;

    let window = web_sys::window().unwrap();
    let resp_value = JsFuture::from(window.fetch_with_request(&request)).await?;

    // `resp_value` is a `Response` object.
    assert!(resp_value.is_instance_of::<Resp>());
    let mut resp: Resp = resp_value.dyn_into().unwrap();

    // Convert this other `Promise` into a rust `Future`.
    let json = JsFuture::from(resp.json()?).await?;

    // Send the JSON response back to JS.
    Ok(json)
}