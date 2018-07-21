extern crate iron;
extern crate params;
extern crate persistent;
extern crate router;
extern crate secure_session;

#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_json;

extern crate base64;
extern crate rand;
extern crate sha2;

extern crate includedir;
extern crate phf;

extern crate chrono;

extern crate handlebars_iron;
extern crate mime_guess;

mod assets;
mod auth;
mod config;
mod io;
mod routes;
mod types;

use auth::*;
use io::*;

use config::Config;

use routes::auth::login;
use routes::auth::logout;
use routes::files::get_pushed_file;
use routes::homepage::homepage;
use routes::manage::manage;
use routes::modify::delete_file;
use routes::modify::rename_file;
use routes::upload::upload;

use iron::prelude::*;
use iron::typemap::Key;
use iron::AroundMiddleware;
use iron::method;

use router::Router;

use secure_session::middleware::{SessionConfig, SessionMiddleware};
use secure_session::session::ChaCha20Poly1305SessionManager;

use handlebars_iron::DirectorySource;
use handlebars_iron::HandlebarsEngine;

#[derive(Copy, Clone)]
struct ConfigContainer;

impl Key for ConfigContainer {
    type Value = Config;
}

fn main() {
    let config = config::load_config("config.json").unwrap();

    let mut key = [0 as u8; 32];

    let mut key_index = 0;
    for byte in config.key.as_bytes() {
        if key_index >= 32 {
            break;
        }

        key[key_index] = *byte;
        key_index += 1;
    }

    let manager = ChaCha20Poly1305SessionManager::<User>::from_key(key);
    let session_config = SessionConfig::default();
    let middleware =
        SessionMiddleware::<User, SessionStore, ChaCha20Poly1305SessionManager<User>>::new(
            manager,
            session_config,
        );

    let mut hbse = HandlebarsEngine::new();
    hbse.add(Box::new(DirectorySource::new("./templates/", ".hbs")));

    hbse.reload().expect("Unable to load templates");

    let phrases = PhraseGenerator::new(
        include_str!("../res/dictionary_adjectives.txt"),
        include_str!("../res/dictionary_nouns.txt"),
    );

    let addr = "127.0.0.1:3000";

    let mut router = Router::new();
    router.route(method::Get, "/", homepage, "homepage");
    router.route(method::Post, "/login", login, "login");
    router.route(method::Get, "/logout", logout, "logout");
    router.route(method::Get, "/manage", manage, "manage");
    router.route(method::Get, "/delete/:file", delete_file, "delete");
    router.route(
        method::Get,
        "/rename/:source/:target",
        rename_file,
        "rename",
    );
    router.route(method::Post, "/upload/:type", upload, "upload");
    router.route(method::Get, "/*", get_pushed_file, "generic_file_handler");

    let mut chain = Chain::new(router);
    chain.link(persistent::Read::<ConfigContainer>::both(config));
    chain.link(persistent::Read::<PhraseGeneratorContainer>::both(phrases));
    chain.link_after(hbse);

    Iron::new(middleware.around(Box::new(chain)))
        .http(addr)
        .unwrap();
}
