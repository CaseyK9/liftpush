#![feature(plugin, decl_macro, custom_derive)]
#![plugin(rocket_codegen)]
extern crate rocket;
extern crate rocket_contrib;

#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_json;

extern crate multipart;

extern crate rand;
extern crate uuid;
extern crate sha2;
extern crate base64;

extern crate includedir;
extern crate phf;

extern crate chrono;

mod assets;
mod auth;
mod io;
mod config;

use rocket::Data;
use rocket::request::Form;
use rocket::request::Request;
use rocket::response;
use rocket::response::{NamedFile, Content, Stream, Redirect, Responder};
use rocket::http::ContentType;
use rocket::http::Cookie;
use rocket::http::Cookies;
use rocket::State;

use rocket_contrib::Template;
use rocket_contrib::Json;

use multipart::server::Multipart;

use std::io::{Read, Write};
use std::fs;
use std::fs::File;
use std::fs::DirEntry;
use std::path::{Path, PathBuf};
use std::error::Error;
use std::sync::Mutex;

use chrono::{DateTime, FixedOffset, Local};

use sha2::Digest;

use assets::FILES as files;
use auth::*;
use io::*;
use config::Config;

#[derive(Serialize)]
struct UploadStatus {
    url : String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
enum FileType {
    File,
    Url,
    Text
}

impl FileType {
    fn from_str(name : &str) -> Option<FileType> {
        match name {
            "file" => Some(FileType::File),
            "url" => Some(FileType::Url),
            "text" => Some(FileType::Text),
            _ => None
        }
    }
}

#[allow(dead_code)]
mod metadata_rfc2822 {
    use chrono::{DateTime, FixedOffset};
    use serde::{self, Deserialize, Serializer, Deserializer};

    pub fn serialize<S>(date: &DateTime<FixedOffset>, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        let s = date.to_rfc2822();
        serializer.serialize_str(&s)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<FixedOffset>, D::Error>
        where D: Deserializer<'de>
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
    file_type : FileType,
    url : Option<String>,
    filename : Option<String>,
    actual_filename : Option<String>
}

#[derive(Serialize)]
struct ManageMetadata {
    name : String,
    meta : FileMetadata
}

impl FileMetadata {
    fn new_from_file(filename : String, actual_filename : String) -> Self {
        Self {
            date : Local::now().with_timezone(&FixedOffset::east(0)),
            file_type : FileType::File,
            filename : Some(filename),
            actual_filename : Some(actual_filename),
            url : None
        }
    }

    fn new_from_text(filename : String, actual_filename : String) -> Self {
        Self {
            date : Local::now().with_timezone(&FixedOffset::east(0)),
            file_type : FileType::Text,
            filename : Some(filename),
            actual_filename : Some(actual_filename),
            url : None
        }
    }

    fn new_from_url(url : String) -> Self {
        Self {
            date : Local::now().with_timezone(&FixedOffset::east(0)),
            file_type : FileType::Url,
            filename : None,
            actual_filename : None,
            url : Some(url)
        }
    }
}

fn parse_meta(file : &str) -> Option<FileMetadata> {
    let meta_filename = "d/".to_string() + file + ".info.json";
    let path = Path::new(&meta_filename);
    let mut meta_file = match File::open(&path) {
        Err(_) => {
            println!("File {} doesn't exist!", file);
            return None;
        },
        Ok(file) => file,
    };

    let mut meta_string = String::new();
    match meta_file.read_to_string(&mut meta_string) {
        Err(_) => {
            println!("File {} is unreadable!", file);
            return None;
        },
        Ok(_) => (),
    }

    let meta : serde_json::Result<FileMetadata> = serde_json::from_str(&meta_string);
    match meta {
        Ok(meta) => Some(meta),
        Err(why) => {
            println!("File {} is not parsable: {}", file, why.description());
            return None;
        }
    }
}

#[post("/<input_type>", data = "<data>", rank = 1)]
fn upload(data : Data, _user : APIUser, boundary : MultipartBoundary, out_file : RandomFilename,
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

#[post("/<_url_type>", data = "<_data>", rank = 2)]
fn upload_no_auth(_url_type : String, _data : Data) -> Result<String, String> {
    Err(format!("Non-authed request"))
}

#[post("/login", data = "<task>")]
fn login(task: Form<LoginAttempt>, mut cookies: Cookies, sessions: State<Mutex<SessionStore>>,
         config : State<Config>) -> Redirect {
    let username = task.get().username.clone();

    let password_str = task.get().password.clone();
    let mut password = sha2::Sha256::default();
    password.input(password_str.as_bytes());
    let password = password.result();
    let password = base64::encode(&password);

    println!("User: {}, password: {}", username, password);

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

    if found {
        let session = sessions.lock().unwrap().new_session(username);
        cookies.add_private(Cookie::new(LOGIN_COOKIE, session));
        Redirect::to(".")
    } else {
        Redirect::to(".?error=invalid-login")
    }
}

#[get("/logout")]
fn logout(_user : User, mut cookies: Cookies, sessions: State<Mutex<SessionStore>>) -> Redirect {
    let api_key = cookies.get_private(LOGIN_COOKIE);
    if api_key.is_some() {
        sessions.lock().unwrap().invalidate_session(api_key.unwrap().value().to_string());
    }

    Redirect::to(".")
}

#[derive(Serialize)]
pub struct FileViewerState {
    username : String,
    files : Vec<ManageMetadata>
}

#[get("/")]
fn manage(user : User) -> Template {
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

    Template::render("manage", &FileViewerState {
        username : user.username,
        files : found_files
    })
}

#[get("/", rank = 2)]
fn homepage() -> Template {
    Template::render("index", &{})
}

#[get("/<file>")]
fn delete_file(_user : User, file: String) -> Option<String> {
    if file.contains(".") || file.contains("/") || file.contains("\\") {
        return None;
    }

    let meta = parse_meta(&file)?;

    match meta.actual_filename {
        Some(name) => fs::remove_file(Path::new("d/").join(name)).unwrap(),
        _ => ()
    }

    fs::remove_file(Path::new("d/").join(file + ".info.json")).unwrap();

    Some(format!("Deleted"))
}

#[get("/<file>/<to>")]
fn rename_file(_user : User, file: String, to : String) -> Option<String> {
    if file.contains(".") || file.contains("/") || file.contains("\\") {
        return None;
    }

    let mut meta = parse_meta(&file)?;

    match meta.actual_filename {
        Some(name) => {
            let new_filename = match name.split(".").next() {
                Some(raw_name) => {
                    let extension_cloned = name.clone();
                    let extension = &extension_cloned[raw_name.len() ..];
                    to.clone() + extension
                }
                _ => {
                    to.clone()
                }
            };

            println!("new filename: {}", new_filename);

            meta.actual_filename = Some(new_filename.clone());
            fs::rename(Path::new("d/").join(name),
                       Path::new("d/").join(&new_filename)).unwrap()
        },
        _ => ()
    }

    fs::remove_file(Path::new("d/").join(file + ".info.json")).unwrap();

    let meta_string = match serde_json::to_string(&meta) {
        Ok(val) => val,
        Err(_) => return None
    };

    let target = to.split(".").next().unwrap().to_string();

    let meta_filename = "d/".to_string() + &target + ".info.json";
    let path = Path::new(&meta_filename);

    let mut meta_file = match File::create(&path) {
        Err(why) => {
            println!("Couldn't create {}: {}",
                     meta_filename,
                     why.description());
            return None;
        },
        Ok(file) => file,
    };

    match meta_file.write_all(meta_string.as_bytes()) {
        Err(why) => {
            println!("Failed to write to {}: {}", meta_filename,
                     why.description());
            return None;
        },
        Ok(_) => (),
    }

    Some(format!("Renamed"))
}

enum FileResponseType {
    File { file : NamedFile },
    Redir { redir : Redirect },
    Template { template : Template }
}

impl<'r> Responder<'r> for FileResponseType {
    fn respond_to(self, req: &Request) -> response::Result<'r> {
        match self {
            FileResponseType::File { file } => file.respond_to(req),
            FileResponseType::Redir { redir } => redir.respond_to(req),
            FileResponseType::Template { template } => template.respond_to(req)
        }
    }
}

#[derive(Serialize)]
struct TextView {
    contents : String,
    meta : FileMetadata
}

#[get("/<file>", rank = 3)]
fn get_pushed_file(file: String) -> Option<FileResponseType> {
    if file.contains("..") || file.contains("/") || file.contains("\\") {
        return None;
    }

    let meta = parse_meta(&file)?;

    match meta.file_type {
        FileType::File => {
            match NamedFile::open(Path::new("d/").join(meta.actual_filename.unwrap())).ok() {
                Some(file) => Some(FileResponseType::File {
                    file
                }),
                _ => None
            }
        },
        FileType::Url => Some(FileResponseType::Redir {
            redir : Redirect::temporary(&meta.url.unwrap())
        }),
        FileType::Text => {
            // Read in text file
            let meta_filename = "d/".to_string() + &meta.actual_filename.clone().unwrap();
            let path = Path::new(&meta_filename);
            let mut meta_file = match File::open(&path) {
                Err(_) => {
                    println!("File {} doesn't exist!", file);
                    return None;
                },
                Ok(file) => file,
            };

            let mut meta_string = String::new();
            match meta_file.read_to_string(&mut meta_string) {
                Err(_) => {
                    println!("File {} is unreadable!", file);
                    return None;
                },
                Ok(_) => (),
            }

            Some(FileResponseType::Template {
                template : Template::render("text", &TextView {
                    contents : meta_string,
                    meta
                })
            })
        },
    }
}

#[get("/<file..>", rank = 4)]
fn static_files(file: PathBuf) -> Option<Content<Stream<Box<Read>>>> {
    let path = file.as_path().to_owned();
    let filename = path.to_str()?;
    let file = match files.read(&("static/".to_owned() + filename)) {
        Ok(val) => val,
        Err(_) => return None
    };

    let content_type = match path.extension() {
        Some(ext) => match ext.to_str() {
            Some(ext) => match ContentType::from_extension(ext) {
                Some(val) => val,
                None => ContentType::Binary
            },
            None => ContentType::Binary
        },
        None => ContentType::Binary
    };

    Some(Content(content_type, Stream::chunked(file, 8192)))
}

fn main() {
    let config = config::load_config("config.json").unwrap();

    let sessions = SessionStore::new();

    let phrases = PhraseGenerator::new(
        include_str!("../res/dictionary_adjectives.txt"),
        include_str!("../res/dictionary_nouns.txt"));

    rocket::ignite()
        .manage(phrases)
        .manage(config)
        .manage(Mutex::new(sessions))
        .mount("/upload", routes![upload, upload_no_auth])
        .mount("/delete", routes![delete_file])
        .mount("/rename", routes![rename_file])
        .mount("/", routes![get_pushed_file, homepage, manage,
         login, logout, static_files])
        .attach(Template::fairing())
        .launch();
}
