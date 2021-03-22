use std::collections::HashMap;

use wapc_guest::prelude::*;
use wasmcloud_actor_http_server as http;
use log::{info, warn, error, trace, debug};

#[wasmcloud_actor_core::init]
pub fn init() {
    http::Handlers::register_handle_request(greet);
    wasmcloud_actor_logging::enable_macros();
}

pub fn greet(_: http::Request) -> HandlerResult<http::Response> {
    info!("info something");
    warn!("warn something");
    error!("error something");
    trace!("trace something");
    debug!("debug something");
    Ok(http::Response {
        status_code: 200,
        status: "OK".to_owned(),
        header: HashMap::new(),
        body: b"Hello, world!\n".to_vec(),
    })
}
