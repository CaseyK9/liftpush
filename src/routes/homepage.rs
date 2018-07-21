//! Contains the homepage endpoint.

use types::StringError;

use auth::SessionStore;

use iron::modifiers::RedirectRaw;
use iron::prelude::*;
use iron::status;

use handlebars_iron::Template;

use params::Params;
use params::Value;

/// The ErrorView is used as parameters to the homepage template.
#[derive(Serialize)]
struct ErrorView {
    error: Option<String>,
}

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

    let error = {
        let map = req.get_ref::<Params>().expect("No Params object available");

        let param: IronResult<&str> = extract_param_type!(map, String, "error");
        param.map(|x| Some(x.to_string())).unwrap_or_else(|_x| None)
    };

    Ok(Response::with((
        status::Ok,
        Template::new("index", &ErrorView { error }),
    )))
}
