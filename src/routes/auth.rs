//! Interactive authentication endpoints.

use ConfigContainer;
use SessionKey;

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

/// Extracts a named value from a Params object, returning IronErrors if these bounds
/// cannot be met.
macro_rules! extract_param_type {
    ($map:ident, $type:ident, $key:expr) => {
        match $map.get($key).ok_or_else(|| {
            IronError::new(
                StringError(format!("Unable to find {} in submitted form", $key)),
                (status::BadRequest, "Missing form params"),
            )
        }) {
            Ok(v) => match v {
                &Value::$type(ref value) => Ok(value),
                _ => Err(IronError::new(
                    StringError(format!("{} isn't the correct type", $key)),
                    (status::BadRequest, "Bad form params"),
                )),
            },
            Err(e) => Err(e),
        }
    };
}

/// Login endpoint. Tests submitted user credentials
///
/// HTTP request required state:
///     Request kind: POST, with key-value pairs of username and password
///     Headers: optional SessionKey
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

    // Respond appropiately
    if found {
        // The user might have a previous session going
        req.extensions.remove::<SessionKey>();
        req.extensions.insert::<SessionKey>(User { username });

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
///     Headers: optional SessionKey
pub fn logout(req: &mut Request) -> IronResult<Response> {
    // This returns an Option type, so no SessionKey will be a no-op here.
    req.extensions.remove::<SessionKey>();

    Ok(Response::with((
        status::Found,
        RedirectRaw(".".to_string()),
    )))
}
