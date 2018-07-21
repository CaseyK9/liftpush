//! Helpers for authentication of users.

use iron::typemap;

/// Stores information about a logged in user.
#[derive(Serialize, Deserialize)]
pub struct User {
    pub username: String,
}

/// The SessionStore stores values of User as sessions.
pub struct SessionStore {}

impl typemap::Key for SessionStore {
    type Value = User;
}
