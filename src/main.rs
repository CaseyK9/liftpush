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
mod types;

use iron::headers::ContentDisposition;
use iron::headers::DispositionParam;
use iron::headers::DispositionType;
use iron::mime::{self, Mime, SubLevel, TopLevel};
use iron::prelude::*;
use iron::typemap::Key;
use iron::AroundMiddleware;
use iron::{typemap, AfterMiddleware, BeforeMiddleware};

use params::Params;

use router::Router;

use secure_session::middleware::{SessionConfig, SessionMiddleware};
use secure_session::session::ChaCha20Poly1305SessionManager;

use std::error::Error;
use std::fs;
use std::fs::DirEntry;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::{DateTime, FixedOffset, Local};

use sha2::Digest;

use assets::FILES as files;
use auth::*;
//use io::*;
use config::Config;
use handlebars_iron::DirectorySource;
use handlebars_iron::HandlebarsEngine;
use handlebars_iron::Template;
use iron::method;
use iron::modifiers::Redirect;
use iron::modifiers::RedirectRaw;
use iron::status;
use iron::Url;
use params::Value;
use types::StringError;

#[derive(Serialize)]
struct UploadStatus {
    url: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
enum FileType {
    File,
    Url,
    Text,
}

impl FileType {
    fn from_str(name: &str) -> Option<FileType> {
        match name {
            "file" => Some(FileType::File),
            "url" => Some(FileType::Url),
            "text" => Some(FileType::Text),
            _ => None,
        }
    }
}

#[allow(dead_code)]
mod metadata_rfc2822 {
    use chrono::{DateTime, FixedOffset};
    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(date: &DateTime<FixedOffset>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = date.to_rfc2822();
        serializer.serialize_str(&s)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<FixedOffset>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        DateTime::parse_from_rfc2822(&s).map_err(serde::de::Error::custom)
    }
}

#[derive(Serialize, Deserialize)]
struct FileMetadata {
    #[serde(with = "metadata_rfc2822")]
    date: DateTime<FixedOffset>, // Mon, 11 Dec 2017 10:28:36 +0000"
    #[serde(rename = "type")]
    file_type: FileType,
    url: Option<String>,
    filename: Option<String>,
    actual_filename: Option<String>,
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

fn parse_meta(file: &str) -> Option<FileMetadata> {
    let meta_filename = "d/".to_string() + file + ".info.json";
    let path = Path::new(&meta_filename);
    let mut meta_file = match File::open(&path) {
        Err(_) => {
            println!("File {} doesn't exist!", file);
            return None;
        }
        Ok(file) => file,
    };

    let mut meta_string = String::new();
    match meta_file.read_to_string(&mut meta_string) {
        Err(_) => {
            println!("File {} is unreadable!", file);
            return None;
        }
        Ok(_) => (),
    }

    let meta: serde_json::Result<FileMetadata> = serde_json::from_str(&meta_string);
    match meta {
        Ok(meta) => Some(meta),
        Err(why) => {
            println!("File {} is not parsable: {}", file, why.description());
            return None;
        }
    }
}

//#[post("/<input_type>", data = "<data>", rank = 1)]
/*fn upload(data : Data, _user : APIUser, boundary : MultipartBoundary, out_file : RandomFilename,
    config : State<Config>, input_type : String)
    -> Result<Json<UploadStatus>, String> {
    let input_type = match FileType::from_str(&input_type) {
        Some(val) => val,
        _ => return Err(format!("Invalid input type"))
    };

    println!("Uploading: {:?}", input_type);

    // Rocket does not support multipart forms (for whatever goddamn reason),
    //  so we directly hook 'multipart' here.
    let mut mp = Multipart::with_body(data.open(), boundary.boundary);

    // We only want to handle the top entry
    let mut entry = match mp.read_entry() {
        Ok(val) => match val {
            Some(val) => val,
            None => return Err(format!("No multipart files found"))
        },
        Err(_) => return Err(format!("Unable to read multipart structure")),
    };

    match entry.data.as_file() {
        Some(file) => {
            // Generate metadata
            let original_filename = match file.filename.clone() {
                Some(filename) => filename,
                _ => return Err(format!("No multipart filename specified"))
            };

            let ext_split = original_filename.clone();
            let ext : Option<&str> = ext_split.split(".").last();

            let url = out_file.filename.clone();

            let new_filename = match ext {
                Some(ext) => out_file.filename + "." + ext,
                _ => out_file.filename
            };

            // Generate metadata
            // TODO: Unwrap
            let meta = match input_type {
                FileType::File => {
                    println!("Save file to {}", new_filename);
                    // TODO: Check to make sure file doesn't exist
                    file.save().with_path("d/".to_string() + &new_filename)
                        .into_result_strict().unwrap();
                    FileMetadata::new_from_file(original_filename,
                                                new_filename.clone())
                },
                FileType::Text => {
                    println!("Save file to {}", new_filename);

                    let mut data = String::new();
                    file.read_to_string(&mut data).unwrap();

                    if data[0 .. 8].contains("://") {
                        FileMetadata::new_from_url(data)
                    } else {
                        // Save buffered text
                        let meta_filename = "d/".to_string() + &new_filename;
                        let path = Path::new(&meta_filename);

                        let mut meta_file = match File::create(&path) {
                            Err(why) => {
                                println!("Couldn't create {}: {}",
                                         meta_filename,
                                         why.description());
                                return Err(format!("Failed to write file"));
                            },
                            Ok(file) => file,
                        };

                        match meta_file.write_all(data.as_bytes()) {
                            Err(why) => {
                                println!("Failed to write to {}: {}", meta_filename,
                                         why.description());
                                return Err(format!("Failed to write file"));
                            },
                            Ok(_) => (),
                        }

                        FileMetadata::new_from_text(original_filename,
                                                    new_filename.clone())
                    }
                },
                _ => { // URL
                    return Err(format!("Type not supported!"));
                }
            };

            let meta_string = match serde_json::to_string(&meta) {
                Ok(data) => data,
                Err(msg) => {
                    println!("Couldn't serialize metadata: {}",
                             msg.description());
                    return Err(format!("Failed to generate metadata"));
                }
            };

            // Save metadata
            let meta_filename = "d/".to_string() + &url + ".info.json";
            let path = Path::new(&meta_filename);

            let mut meta_file = match File::create(&path) {
                Err(why) => {
                    println!("Couldn't create {}: {}",
                             meta_filename,
                             why.description());
                    return Err(format!("Failed to write file"));
                },
                Ok(file) => file,
            };

            match meta_file.write_all(meta_string.as_bytes()) {
                Err(why) => {
                    println!("Failed to write to {}: {}", meta_filename,
                           why.description());
                    return Err(format!("Failed to write file"));
                },
                Ok(_) => ()
            }

            Ok(Json(UploadStatus {
                url: config.base_url.clone() + &url
            }))
        }
        _ => Err(format!("Multipart segment was not file"))
    }
}

//#[post("/<_url_type>", data = "<_data>", rank = 2)]
fn upload_no_auth(_url_type : String, _data : Data) -> Result<String, String> {
    Err(format!("Non-authed request"))
}*/

//#[post("/login", data = "<task>")]
fn login(req: &mut Request) -> IronResult<Response> {
    let (username, password) = {
        let map = req.get_ref::<Params>().unwrap();
        let username = map.get("username").ok_or_else(|| {
            IronError::new(
                StringError("Unable to find username in submitted form".into()),
                (status::BadRequest, "Missing form params"),
            )
        })?;

        let username = match username {
            &Value::String(ref str) => str,
            _ => {
                return Err(IronError::new(
                    StringError("Username isn't a string!".into()),
                    (status::BadRequest, "Bad form params"),
                ))
            }
        };

        let password_str = map.get("password").ok_or_else(|| {
            IronError::new(
                StringError("Unable to find username in submitted form".into()),
                (status::BadRequest, "Missing form params"),
            )
        })?;

        let password_str = match password_str {
            &Value::String(ref str) => str,
            _ => {
                return Err(IronError::new(
                    StringError("Password isn't a string!".into()),
                    (status::BadRequest, "Bad form params"),
                ))
            }
        };

        let mut password = sha2::Sha256::default();
        password.input(password_str.as_bytes());
        let password = password.result();
        let password = base64::encode(&password);

        (username.to_string(), password)
    };

    println!("User: {}, password: {}", username, password);

    let arc = req.get::<persistent::Read<ConfigContainer>>().unwrap();
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

    println!("Found? {}", found);

    if found {
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

//#[get("/logout")]
fn logout(req: &mut Request) -> IronResult<Response> {
    req.extensions.remove::<SessionKey>();

    Ok(Response::with((
        status::Found,
        RedirectRaw(".".to_string()),
    )))
}

#[derive(Serialize)]
pub struct FileViewerState {
    username : String,
    files : Vec<ManageMetadata>
}

fn manage(req: &mut Request) -> IronResult<Response> {
    let user = req.extensions.get::<SessionKey>().ok_or_else(|| {
        IronError::new(
            StringError("User attempted to access restricted page".into()),
            (status::Unauthorized, "You are not logged in"),
        )
    })?;

    let paths = fs::read_dir("d").unwrap();

    let mut found_files : Vec<ManageMetadata> = Vec::new();

    for path in paths {
        let path : DirEntry = path.unwrap();
        let path_filename = path.file_name();
        let filename = path_filename.to_str().unwrap();

        if filename.ends_with(".info.json") {
            let name : &str = filename.split(".").next().unwrap();
            if let Some(meta) = parse_meta(name) {
                found_files.push(ManageMetadata {
                    name : name.to_string(),
                    meta
                });
            }
        }
    }

    found_files.sort_by(|a, b|
        b.meta.date.partial_cmp(&a.meta.date).unwrap());

    Ok(Response::with((status::Ok, Template::new("manage", &FileViewerState {
        username : user.username.to_owned(),
        files : found_files
    }))))
}

fn delete_file(req: &mut Request) -> IronResult<Response> {
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
        return Ok(Response::with((status::NotFound)));
    }

    let meta = match parse_meta(&file) {
        Some(v) => v,
        None => return Ok(Response::with((status::NotFound)))
    };

    match meta.actual_filename {
        Some(name) => fs::remove_file(Path::new("d/").join(name)).unwrap(),
        _ => {}
    }

    fs::remove_file(Path::new("d/").join(format!("{}.info.json", file))).unwrap();

    Ok(Response::with((status::Ok, "Deleted")))
}

fn rename_file(req: &mut Request) -> IronResult<Response> {
    let router = req
        .extensions
        .get::<Router>()
        .unwrap();

    let file = router
        .find("source")
        .ok_or_else(|| {
            IronError::new(
                StringError("No source file specified for rename operation".into()),
                (status::NotFound, "No source file specified"),
            )
        })?;

    let to = router
        .find("target")
        .ok_or_else(|| {
            IronError::new(
                StringError("No target file specified for rename operation".into()),
                (status::NotFound, "No target file specified"),
            )
        })?;

    if file.contains(".") || file.contains("/") || file.contains("\\") {
        return Ok(Response::with((status::NotFound)));
    }

    let mut meta = match parse_meta(&file) {
        Some(v) => v,
        None => return Ok(Response::with((status::NotFound)))
    };

    if to.contains(".") || to.contains("/") || to.contains("\\") {
        return Ok(Response::with((status::NotFound)));
    }

    match meta.actual_filename {
        Some(name) => {
            let new_filename = match name.split(".").next() {
                Some(raw_name) => {
                    let extension_cloned = name.clone();
                    let extension = &extension_cloned[raw_name.len() ..];
                    to.to_owned() + extension
                }
                _ => {
                    to.to_owned()
                }
            };

            println!("new filename: {}", new_filename);

            meta.actual_filename = Some(new_filename.clone());
            fs::rename(Path::new("d/").join(name),
                       Path::new("d/").join(&new_filename)).unwrap()
        },
        _ => ()
    }

    fs::remove_file(Path::new("d/").join(format!("{}.info.json", file))).unwrap();

    let meta_string = match serde_json::to_string(&meta) {
        Ok(val) => val,
        Err(_) => return Ok(Response::with((status::NotFound)))
    };

    let target = to.split(".").next().unwrap().to_string();

    let meta_filename = "d/".to_string() + &target + ".info.json";
    let path = Path::new(&meta_filename);

    let mut meta_file = match File::create(&path) {
        Err(why) => {
            println!("Couldn't create {}: {}",
                     meta_filename,
                     why.description());
            return Ok(Response::with((status::NotFound)));
        },
        Ok(file) => file,
    };

    match meta_file.write_all(meta_string.as_bytes()) {
        Err(why) => {
            println!("Failed to write to {}: {}", meta_filename,
                     why.description());
            return Ok(Response::with((status::NotFound)));
        },
        Ok(_) => (),
    }

    Ok(Response::with((status::Ok, "Renamed")))
}

fn homepage(req: &mut Request) -> IronResult<Response> {
    if req.extensions.get::<SessionKey>().is_some() {
        return
            Ok(Response::with((
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
}

fn get_static_file(path: &str) -> Option<(Vec<u8>, mime::Mime)> {
    println!("Matching patch: {}", path);
    let path = PathBuf::from(&path).to_owned();
    // TODO: Don't unwrap
    let filename = path.to_str().unwrap();

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
            let size = file.read_to_end(&mut buffer).unwrap();

            Some((buffer, content_type))
        }
        Err(_) => None,
    }
}

fn get_pushed_file(req: &mut Request) -> IronResult<Response> {
    let ref path = req
        .extensions
        .get::<Router>()
        .unwrap()
        .find("")
        .unwrap_or("");

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
        return Ok(Response::with((status::NotFound)));
    }

    let meta = match parse_meta(&path) {
        Some(v) => v,
        _ => return Ok(Response::with((status::NotFound))),
    };

    match meta.file_type {
        FileType::File => {
            let file = Path::new("d/").join(meta.actual_filename.unwrap());

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
                return Ok(Response::with((status::NotFound)));
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
            // Read in text file
            let meta_filename = "d/".to_string() + &meta.actual_filename.clone().unwrap();
            let path = Path::new(&meta_filename);
            let mut meta_file = match File::open(&path) {
                Err(_) => {
                    println!("File {:?} doesn't exist!", path);
                    return Ok(Response::with((status::NotFound)));
                }
                Ok(file) => file,
            };

            let mut meta_string = String::new();
            match meta_file.read_to_string(&mut meta_string) {
                Err(_) => {
                    println!("File {:?} is unreadable!", path);
                    return Ok(Response::with((status::NotFound)));
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

    /*let phrases = PhraseGenerator::new(
        include_str!("../res/dictionary_adjectives.txt"),
        include_str!("../res/dictionary_nouns.txt"));*/

    let addr = "127.0.0.1:8080";

    let mut router = Router::new();
    router.route(method::Get, "/", homepage, "homepage");
    router.route(method::Post, "/login", login, "login");
    router.route(method::Get, "/logout", logout, "logout");
    router.route(method::Get, "/manage", manage, "manage");
    router.route(method::Get, "/delete/:file", delete_file, "delete");
    router.route(method::Get, "/rename/:source/:target", rename_file, "rename");
    router.route(method::Get, "/*", get_pushed_file, "generic_file_handler");

    let mut chain = Chain::new(router);
    //chain.link_after(Custom404);
    chain.link(persistent::Read::<ConfigContainer>::both(config));
    chain.link_after(hbse);

    Iron::new(middleware.around(Box::new(chain)))
        .http("localhost:3000")
        .unwrap();

}
