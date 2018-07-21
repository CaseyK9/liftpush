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
extern crate uuid;

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

use iron::headers::ContentDisposition;
use iron::headers::DispositionParam;
use iron::headers::DispositionType;
use iron::mime::{self, Mime, SubLevel, TopLevel};
use iron::prelude::*;
use iron::typemap;
use iron::typemap::Key;
use iron::AroundMiddleware;

use params::Params;

use router::Router;

use secure_session::middleware::{SessionConfig, SessionMiddleware};
use secure_session::session::ChaCha20Poly1305SessionManager;

use std::error::Error;
use std::fs;
use std::fs::copy;
use std::fs::DirEntry;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use chrono::{DateTime, FixedOffset, Local};

use sha2::Digest;

use assets::FILES as files;
use auth::*;
use config::Config;
use handlebars_iron::DirectorySource;
use handlebars_iron::HandlebarsEngine;
use handlebars_iron::Template;
use io::*;
use iron::method;
use iron::modifiers::Redirect;
use iron::modifiers::RedirectRaw;
use iron::status;
use iron::Url;
use params::Value;

use types::FileMetadata;
use types::StringError;

use routes::auth::login;
use routes::auth::logout;
use routes::upload::upload;

use types::FileType;

#[derive(Serialize)]
struct UploadStatus {
    url: String,
}

#[derive(Serialize)]
struct ManageMetadata {
    name: String,
    meta: FileMetadata,
}

impl FileMetadata {
    fn new_from_file(filename: String, actual_filename: String) -> Self {
        Self {
            date: Local::now().with_timezone(&FixedOffset::east(0)),
            file_type: FileType::File,
            filename: Some(filename),
            actual_filename: Some(actual_filename),
            url: None,
        }
    }

    fn new_from_text(filename: String, actual_filename: String) -> Self {
        Self {
            date: Local::now().with_timezone(&FixedOffset::east(0)),
            file_type: FileType::Text,
            filename: Some(filename),
            actual_filename: Some(actual_filename),
            url: None,
        }
    }

    fn new_from_url(url: String) -> Self {
        Self {
            date: Local::now().with_timezone(&FixedOffset::east(0)),
            file_type: FileType::Url,
            filename: None,
            actual_filename: None,
            url: Some(url),
        }
    }
}

#[derive(Serialize)]
pub struct FileViewerState {
    username: String,
    files: Vec<ManageMetadata>,
}

fn manage(req: &mut Request) -> IronResult<Response> {
    let base_path = {
        let arc = req.get::<persistent::Read<ConfigContainer>>().unwrap();
        let config = arc.as_ref();
        config.base_path.to_owned()
    };

    let user = req.extensions.get::<SessionKey>().ok_or_else(|| {
        IronError::new(
            StringError("User attempted to access restricted page".into()),
            (status::Unauthorized, "You are not logged in"),
        )
    })?;

    let paths = fs::read_dir("d").unwrap();

    let mut found_files: Vec<ManageMetadata> = Vec::new();

    for path in paths {
        let path: DirEntry = path.unwrap();
        let path_filename = path.file_name();
        let filename = path_filename.to_str().unwrap();

        if filename.ends_with(".info.json") {
            let name: &str = filename.split(".").next().unwrap();

            match FileMetadata::from_path(&base_path, &name) {
                Ok(meta) => {
                    found_files.push(ManageMetadata {
                        name: name.to_string(),
                        meta,
                    });
                }
                Err(v) => eprintln!("Failed to open file {:?}: {:?}", filename, v),
            }
        }
    }

    found_files.sort_by(|a, b| b.meta.date.partial_cmp(&a.meta.date).unwrap());

    Ok(Response::with((
        status::Ok,
        Template::new(
            "manage",
            &FileViewerState {
                username: user.username.to_owned(),
                files: found_files,
            },
        ),
    )))
}

fn delete_file(req: &mut Request) -> IronResult<Response> {
    let base_path = {
        let arc = req.get::<persistent::Read<ConfigContainer>>().unwrap();
        let config = arc.as_ref();
        config.base_path.to_owned()
    };

    let file = req
        .extensions
        .get::<Router>()
        .unwrap()
        .find("file")
        .ok_or_else(|| {
            IronError::new(
                StringError("No file specified for delete operation".into()),
                (status::NotFound, "No file specified"),
            )
        })?;

    if file.contains(".") || file.contains("/") || file.contains("\\") {
        return Ok(Response::with(status::NotFound));
    }

    let meta = FileMetadata::from_path(&base_path, &file).map_err(|x| {
        IronError::new(
            StringError(x),
            (status::NotFound, "Failed to find metadata"),
        )
    })?;

    match meta.actual_filename {
        Some(name) => fs::remove_file(Path::new(&base_path).join(name)).unwrap(),
        _ => {}
    }

    fs::remove_file(Path::new(&base_path).join(format!("{}.info.json", file))).unwrap();

    Ok(Response::with((status::Ok, "Deleted")))
}

fn rename_file(req: &mut Request) -> IronResult<Response> {
    let base_path = {
        let arc = req.get::<persistent::Read<ConfigContainer>>().unwrap();
        let config = arc.as_ref();
        config.base_path.to_owned()
    };

    let router = req.extensions.get::<Router>().unwrap();

    let file = router.find("source").ok_or_else(|| {
        IronError::new(
            StringError("No source file specified for rename operation".into()),
            (status::NotFound, "No source file specified"),
        )
    })?;

    let to = router.find("target").ok_or_else(|| {
        IronError::new(
            StringError("No target file specified for rename operation".into()),
            (status::NotFound, "No target file specified"),
        )
    })?;

    if file.contains(".") || file.contains("/") || file.contains("\\") {
        return Ok(Response::with(status::NotFound));
    }

    let mut meta = FileMetadata::from_path(&base_path, &file).map_err(|x| {
        IronError::new(
            StringError(x),
            (status::NotFound, "Failed to find metadata"),
        )
    })?;

    if to.contains(".") || to.contains("/") || to.contains("\\") {
        return Ok(Response::with(status::NotFound));
    }

    match meta.actual_filename.take() {
        Some(name) => {
            let new_filename = match name.split(".").next() {
                Some(raw_name) => {
                    let extension_cloned = name.clone();
                    let extension = &extension_cloned[raw_name.len()..];
                    to.to_owned() + extension
                }
                _ => to.to_owned(),
            };

            println!("new filename: {}", new_filename);

            meta.actual_filename = Some(new_filename.clone());
            fs::rename(
                Path::new(&base_path).join(name),
                Path::new(&base_path).join(&new_filename),
            ).unwrap()
        }
        _ => (),
    }

    fs::remove_file(Path::new(&base_path).join(format!("{}.info.json", file))).unwrap();

    let meta_string = match serde_json::to_string(&meta) {
        Ok(val) => val,
        Err(_) => return Ok(Response::with(status::NotFound)),
    };

    let target = to.split(".").next().unwrap().to_string();

    let meta_filename = base_path + &target + ".info.json";
    let path = Path::new(&meta_filename);

    let mut meta_file = match File::create(&path) {
        Err(why) => {
            println!("Couldn't create {}: {}", meta_filename, why.description());
            return Ok(Response::with(status::NotFound));
        }
        Ok(file) => file,
    };

    match meta_file.write_all(meta_string.as_bytes()) {
        Err(why) => {
            println!(
                "Failed to write to {}: {}",
                meta_filename,
                why.description()
            );
            return Ok(Response::with(status::NotFound));
        }
        Ok(_) => (),
    }

    Ok(Response::with((status::Ok, "Renamed")))
}

fn homepage(req: &mut Request) -> IronResult<Response> {
    if req.extensions.get::<SessionKey>().is_some() {
        return Ok(Response::with((
            status::Found,
            RedirectRaw("manage".to_string()),
        )));
    }

    Ok(Response::with((status::Ok, Template::new("index", {}))))
}

#[derive(Serialize)]
struct TextView {
    contents: String,
    meta: FileMetadata,
    url: String,
}

fn get_static_file(filename: &str) -> Option<(Vec<u8>, mime::Mime)> {
    let path = PathBuf::from(&filename).to_owned();

    match files.read(&("static/".to_owned() + filename)) {
        Ok(mut file) => {
            let content_type: mime::Mime = match path.extension() {
                Some(ext) => match ext.to_str() {
                    Some(ext) => match mime_guess::get_mime_type_opt(ext) {
                        Some(v) => v,
                        None => Mime(TopLevel::Application, SubLevel::OctetStream, Vec::new()),
                    },
                    None => Mime(TopLevel::Application, SubLevel::OctetStream, Vec::new()),
                },
                None => Mime(TopLevel::Application, SubLevel::OctetStream, Vec::new()),
            };

            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer).unwrap();

            Some((buffer, content_type))
        }
        Err(_) => None,
    }
}

fn get_pushed_file(req: &mut Request) -> IronResult<Response> {
    let base_path = {
        let arc = req.get::<persistent::Read<ConfigContainer>>().unwrap();
        let config = arc.as_ref();
        config.base_path.to_owned()
    };

    let path = req
        .extensions
        .get::<Router>()
        .unwrap()
        .find("")
        .unwrap_or("")
        .to_owned();

    // Firstly, see if this is a static file
    let file = get_static_file(&path);

    match file {
        // Send static file
        Some((buffer, content_type)) => {
            return Ok(Response::with((content_type, status::Ok, buffer)));
        }
        _ => {}
    }

    if path.contains("..") || path.contains("/") || path.contains("\\") {
        return Ok(Response::with(status::NotFound));
    }

    let meta = FileMetadata::from_path(&base_path, &path).map_err(|x| {
        IronError::new(
            StringError(x),
            (status::NotFound, "Failed to find metadata"),
        )
    })?;

    match meta.file_type {
        FileType::File => {
            let file = Path::new(&base_path).join(meta.actual_filename.unwrap());

            if file.exists() {
                let content_type = mime_guess::guess_mime_type(&file);

                let mut response = Response::with((content_type, status::Ok, file));
                response.headers.set(ContentDisposition {
                    disposition: DispositionType::Inline,
                    parameters: vec![DispositionParam::Ext(
                        format!("filename"),
                        meta.filename
                            .clone()
                            .expect("Should have filename for File type"),
                    )],
                });
                return Ok(response);
            } else {
                return Ok(Response::with(status::NotFound));
            }
        }
        FileType::Url => {
            // TODO: URLs
            return Ok(Response::with((
                status::Found,
                Redirect(Url::parse(&meta.url.unwrap()).map_err(|x| {
                    IronError::new(
                        StringError(x),
                        (status::InternalServerError, "Unable to build target URL"),
                    )
                })?),
            )));
        }
        FileType::Text => {
            let base_url = {
                let arc = req.get::<persistent::Read<ConfigContainer>>().unwrap();
                let config = arc.as_ref();
                config.base_url.to_owned()
            };

            let url = base_url + &path;

            // Read in text file
            let meta_filename = base_path.to_string() + &meta.actual_filename.clone().unwrap();
            let path = Path::new(&meta_filename);
            let mut meta_file = match File::open(&path) {
                Err(_) => {
                    println!("File {:?} doesn't exist!", path);
                    return Ok(Response::with(status::NotFound));
                }
                Ok(file) => file,
            };

            let mut meta_string = String::new();
            match meta_file.read_to_string(&mut meta_string) {
                Err(_) => {
                    println!("File {:?} is unreadable!", path);
                    return Ok(Response::with(status::NotFound));
                }
                Ok(_) => (),
            }

            return Ok(Response::with((
                status::Ok,
                Template::new(
                    "text",
                    &TextView {
                        contents: meta_string,
                        meta,
                        url,
                    },
                ),
            )));
        }
    }
}

#[derive(Copy, Clone)]
struct ConfigContainer;

impl Key for ConfigContainer {
    type Value = Config;
}

struct SessionKey {}

impl typemap::Key for SessionKey {
    type Value = User;
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
        SessionMiddleware::<User, SessionKey, ChaCha20Poly1305SessionManager<User>>::new(
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
