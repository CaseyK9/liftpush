//! Contains the homepage endpoint.

use SessionKey;

use iron::modifiers::RedirectRaw;
use iron::prelude::*;
use iron::status;

use handlebars_iron::Template;

/// Homepage endpoint. Displays a login page if required, else redirects to management
/// endpoint.
///
/// HTTP request required state:
///     Request kind: GET
///     Headers: optional SessionKey
pub fn homepage(req: &mut Request) -> IronResult<Response> {
    if req.extensions.get::<SessionKey>().is_some() {
        return Ok(Response::with((
            status::Found,
            RedirectRaw("manage".to_string()),
        )));
    }

    Ok(Response::with((status::Ok, Template::new("index", {}))))
}
