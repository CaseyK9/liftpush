//! Contains all the routes used by Iron.

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

pub mod auth;
pub mod homepage;
pub mod manage;
pub mod modify;
pub mod upload;
