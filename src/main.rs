//! The main entrypoint for the application. Contains the main function for initialisation
//! of the webserver.
#![forbid(unsafe_code)]
#![feature(plugin)]
#![plugin(phf_macros)]

extern crate handlebars;
extern crate handlebars_iron;
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
extern crate tiny_keccak;

extern crate phf;

extern crate chrono;

extern crate mime_guess;

mod assets;
mod auth;
mod config;
mod rng;
mod routes;
mod splitter;
mod types;

use auth::*;
use rng::*;

use assets::get_file;
use assets::list_files;
use assets::TEMPLATES as templates;

use config::Config;
use config::ConfigContainer;

use routes::auth::login;
use routes::auth::logout;
use routes::files::get_pushed_file;
use routes::homepage::homepage;
use routes::manage::listing;
use routes::manage::manage;
use routes::modify::delete_file;
use routes::modify::rename_file;
use routes::upload::upload;

use splitter::ChainSplit;

use iron::method;
use iron::prelude::*;
use iron::AroundMiddleware;

use router::Router;

use secure_session::middleware::{SessionConfig, SessionMiddleware};
use secure_session::session::ChaCha20Poly1305SessionManager;

use handlebars::Handlebars;

use handlebars_iron::HandlebarsEngine;

/// The main entrypoint for the application.
fn main() {
    let config = Config::from_file("config.json").expect("Unable to load configuration");
    let bind_addr = config.bind_addr.to_owned();

    // Generate the crypto-key used for sessions, sourced from the configuration key.
    let mut key = [0 as u8; 32];

    let mut key_index = 0;
    for byte in config.key.as_bytes() {
        if key_index >= 32 {
            break;
        }

        key[key_index] = *byte;
        key_index += 1;
    }

    // Setup the middleware, consuming the main key.
    let manager = ChaCha20Poly1305SessionManager::<User>::from_key(key);
    let session_config = SessionConfig::default();
    let middleware =
        SessionMiddleware::<User, SessionStore, ChaCha20Poly1305SessionManager<User>>::new(
            manager,
            session_config,
        );

    // Start the templating engine
    let mut handlebars = Handlebars::new();
    for file in list_files(&templates) {
        // Transform "templates/text.hbs" to "text"
        let template_name = file
            .split(".")
            .next()
            .expect("No filename found when one was expected");

        println!("Loading {:?} from {:?}", template_name, file);

        handlebars
            .register_template_source(
                template_name,
                &mut get_file(&templates, file).expect("Unable to find indexed value"),
            )
            .expect("Unable to load template");
    }

    let hbse = HandlebarsEngine::from(handlebars);

    // Generate the RNG
    let phrases = PhraseGenerator::new(
        include_str!("../res/dictionary_adjectives.txt"),
        include_str!("../res/dictionary_nouns.txt"),
    );

    // Build the primary router

    // Authenticated endpoints - this sets a cookie, which could normally have privacy concerns.
    let mut router = Router::new();
    router.route(method::Get, "/", homepage, "homepage");
    router.route(method::Post, "/login", login, "login");
    router.route(method::Get, "/logout", logout, "logout");
    router.route(method::Get, "/manage", manage, "manage");
    router.route(method::Get, "/listing", listing, "listing");
    router.route(method::Get, "/delete/:file", delete_file, "delete");
    router.route(
        method::Get,
        "/rename/:source/:target",
        rename_file,
        "rename",
    );
    router.route(method::Post, "/upload/:type", upload, "upload");

    // Non-authenticated endpoints - no cookies here.
    let mut router_no_cookie = Router::new();
    router_no_cookie.route(method::Get, "/*", get_pushed_file, "generic_file_handler");

    // Splitter delegates between authenticated and non-authenticated endpoints.
    let split = ChainSplit::new(
        middleware.around(Box::new(router)),
        router_no_cookie,
        vec!["delete/", "upload/", "rename/"]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        vec!["", "login", "logout", "manage", "listing"]
            .iter()
            .map(|s| s.to_string())
            .collect(),
    );

    // Final chain adds on general metadata.
    let mut chain = Chain::new(split);
    chain.link(persistent::Read::<ConfigContainer>::both(config));
    chain.link(persistent::Read::<PhraseGeneratorContainer>::both(phrases));
    chain.link_after(hbse);

    println!("Starting server on {:?}...", bind_addr);

    Iron::new(chain)
        .http(bind_addr)
        .expect("Unable to start up webserver");
}
