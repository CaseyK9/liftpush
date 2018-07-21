//! Interactive authentication endpoints.

use config::ConfigContainer;

use auth::SessionStore;

use types::StringError;

use auth::User;

use iron::modifiers::RedirectRaw;
use iron::prelude::*;
use iron::status;

use params::Params;
use params::Value;

use sha2;
use sha2::Digest;

use base64;

use persistent;

/// Login endpoint. Tests submitted user credentials
///
/// HTTP request required state:
///     Request kind: POST, with key-value pairs of username and password
///     Headers: optional SessionStore
pub fn login(req: &mut Request) -> IronResult<Response> {
    // Grab the username and password from the POST request
    let (username, password) = {
        let map = req.get_ref::<Params>().expect("No Params object available");
        let username = extract_param_type!(map, String, "username")?;
        let password_str = extract_param_type!(map, String, "password")?;

        // Hash the inputted password
        let mut password = sha2::Sha256::default();
        password.input(password_str.as_bytes());
        let password = password.result();
        let password = base64::encode(&password);

        (username.to_string(), password)
    };

    println!("User {:?} attempted login", username);

    let arc = req
        .get::<persistent::Read<ConfigContainer>>()
        .expect("No Confguration object available");
    let config = arc.as_ref();

    // Find target user
    let mut found = false;

    for potential_user in &config.users {
        if potential_user.username == username {
            if potential_user.password == password {
                found = true;
            }
            break;
        }
    }

    // Respond appropriately
    if found {
        // The user might have a previous session going
        req.extensions.remove::<SessionStore>();
        req.extensions.insert::<SessionStore>(User { username });

        Ok(Response::with((
            status::Found,
            RedirectRaw("manage".to_string()),
        )))
    } else {
        Ok(Response::with((
            status::Found,
            RedirectRaw(".?error=invalid-login".to_string()),
        )))
    }
}

/// Logout endpoint. Will strip session metadata (if it exists),
/// and redirects back to the home pages.
///
/// HTTP request required state:
///     Request kind: GET
///     Headers: optional SessionStore
pub fn logout(req: &mut Request) -> IronResult<Response> {
    // This returns an Option type, so no SessionStore will be a no-op here.
    req.extensions.remove::<SessionStore>();

    Ok(Response::with((
        status::Found,
        RedirectRaw(".".to_string()),
    )))
}
