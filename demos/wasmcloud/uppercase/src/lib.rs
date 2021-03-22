use log::info;
use serde::Serialize;
use wapc_guest::prelude::*;
use wasmcloud_actor_http_server as http;

#[wasmcloud_actor_core::init]
pub fn init() {
    http::Handlers::register_handle_request(uppercase);
}

fn uppercase(r: http::Request) -> HandlerResult<http::Response> {
    info!("Query String: {}", r.query_string);
    let upper = UppercaseResponse {
        original: r.query_string.to_string(),
        uppercased: r.query_string.to_ascii_uppercase(),
    };

    Ok(http::Response::json(upper, 200, "OK"))
}

#[derive(Serialize)]
struct UppercaseResponse {
    original: String,
    uppercased: String,
}
