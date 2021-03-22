use wapc_guest::prelude::*;
use wasmcloud_actor_blobstore as blobstore;
use wasmcloud_actor_http_server as http;

#[wasmcloud_actor_core::init]
pub fn init() {
    http::Handlers::register_handle_request(fetch);
}

fn fetch(r: http::Request) -> HandlerResult<http::Response> {
    // k8s volumes are mounted into the wasmCloud runtime using the same volume mount name
    let store = blobstore::host("storage");
    let mut path = String::from(r.path);

    // strip the leading slash from the path
    path = path.trim_start_matches('/').to_string();

    match r.method.as_str() {
        "GET" => {
            let blob = store.get_object_info(path.as_str().to_owned(), String::default())?;
            if blob.id == "none" {
                return Ok(http::Response::not_found());
            }
            Ok(http::Response::json(blob, 200, "OK"))
        }
        "POST" => {
            let mut chunk = blobstore::FileChunk {
                id: path,
                container: blobstore::Container::new(""),
                sequence_no: 0,
                total_bytes: r.body.len() as u64,
                chunk_size: r.body.len() as u64,
                context: None,
                chunk_bytes: Vec::with_capacity(0),
            };
            // TODO: check if this is the start of an upload or another chunk. Right now we accept the request as the only chunk.
            store.start_upload(chunk.clone())?;
            chunk.chunk_bytes = r.body;
            store.upload_chunk(chunk)?;
            Ok(http::Response::ok())
        }
        "DELETE" => {
            store.remove_object(path.as_str().to_owned(), String::default())?;
            Ok(http::Response::ok())
        }
        _ => Ok(http::Response::bad_request()),
    }
}
