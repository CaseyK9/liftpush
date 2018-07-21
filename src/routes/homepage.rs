//! Contains the homepage endpoint.

use auth::SessionStore;

use iron::modifiers::RedirectRaw;
use iron::prelude::*;
use iron::status;

use handlebars_iron::Template;

/// Homepage endpoint. Displays a login page if required, else redirects to management
/// endpoint.
///
/// HTTP request required state:
///     Request kind: GET
///     Headers: optional SessionStore
pub fn homepage(req: &mut Request) -> IronResult<Response> {
    if req.extensions.get::<SessionStore>().is_some() {
        return Ok(Response::with((
            status::Found,
            RedirectRaw("manage".to_string()),
        )));
    }

    Ok(Response::with((status::Ok, Template::new("index", {}))))
}
