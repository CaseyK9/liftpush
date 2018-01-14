use rocket::Outcome::*;
use rocket::request::{Request, Outcome, FromRequest};
use rocket::http::Status;
use rocket::State;

use uuid::Uuid;

use std::collections::HashMap;
use std::sync::Mutex;

use config;

pub static LOGIN_HEADER : &'static str = "X-API-Key";
pub static LOGIN_COOKIE : &'static str = "token";

pub struct SessionStore {
    sessions : HashMap<Uuid, String>
}

impl SessionStore {
    pub fn new_session(&mut self, user : String) -> String {
        let uuid = Uuid::new_v4();
        self.sessions.insert(uuid.clone(), user);
        uuid.to_string()
    }

    pub fn valid_session(&self, session : String) -> Option<String> {
        let uuid = match Uuid::parse_str(&session) {
            Ok(uuid) => uuid,
            Err(_) => return None
        };

        match self.sessions.get(&uuid) {
            Some(val) => Some(val.to_string()),
            None => None
        }
    }

    pub fn invalidate_session(&mut self, session : String) -> Option<String> {
        let uuid = match Uuid::parse_str(&session) {
            Ok(uuid) => uuid,
            Err(_) => return None
        };

        self.sessions.remove(&uuid)
    }

    pub fn new() -> Self {
        Self {
            sessions : HashMap::new()
        }
    }
}

#[derive(FromForm)]
pub struct LoginAttempt {
    pub username : String,
    pub password : String
}

pub struct APIUser {
    // TODO!
}

impl<'a, 'r> FromRequest<'a, 'r>  for APIUser {
    type Error = ();

    fn from_request(request: &'a Request<'r>) -> Outcome<Self, Self::Error> {
        let config : State<config::Config> = match request.guard() {
            Success(config) => config,
            _ => return Failure((Status::ServiceUnavailable, ()))
        };

        // Check for API key
        let api_key = request.headers().get_one(LOGIN_HEADER);
        if api_key.is_some() {
            let api_key = api_key.unwrap();

            for key in &config.api_keys {
                if key.key == api_key {
                    return Success(Self {});
                }
            }

            return Failure((Status::Unauthorized, ()));
        }

        Forward(())
    }
}

pub struct User {
    pub username : String
    // TODO!
}

impl<'a, 'r> FromRequest<'a, 'r>  for User {
    type Error = ();

    fn from_request(request: &'a Request<'r>) -> Outcome<Self, Self::Error> {
        let config : State<Mutex<SessionStore>> = match request.guard() {
            Success(config) => config,
            _ => return Failure((Status::ServiceUnavailable, ()))
    };

        // Check for cookie
        let api_key = request.cookies().get_private(LOGIN_COOKIE);
        if api_key.is_some() {
            if let Some(session) =
            config.lock().unwrap().valid_session(api_key.unwrap().value().to_string()) {
                return Success(Self {
                    username : session
                });
            }
        }

        Forward(())
    }
}