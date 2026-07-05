use crate::middleware::app::RequestApp;
use crate::middleware::log_request::RequestLogExt;
use crate::util::RequestPartsExt;
use crate::util::errors::{AppResult, forbidden};
use http::request::Parts;
use http::header;

/// The Origin header (<https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Origin>)
/// is sent with CORS requests and POST requests, and indicates where the request comes from.
///
/// We don't want to accept authenticated requests that originated from other sites, so this
/// function returns an error if the Origin header doesn't match what we expect "this site" to
/// be: <https://crates.io> in production, or <http://localhost:port/> in development.
pub fn verify_origin(parts: &Parts) -> AppResult<()> {
    let headers = parts.headers();
    let allowed_origins = &parts.app().config.allowed_origins;

    let bad_origin = headers
        .get_all(header::ORIGIN)
        .iter()
        .find(|value| !allowed_origins.contains(value));

    if let Some(bad_origin) = bad_origin {
        let error_message =
            format!("only same-origin requests can be authenticated. got {bad_origin:?}");

        parts.request_log().add("cause", error_message);

        return Err(forbidden("invalid origin header"));
    }
    Ok(())
}

