//! Contains the endpoint for browsing static and uploaded files + helpers.

use config::ConfigContainer;

use types::FileMetadata;
use types::FileType;
use types::StringError;

use assets::get_file;
use assets::FILES as files;

use std::fs::File;
use std::io::Read;
use std::path::Path;

use iron::headers::ContentDisposition;
use iron::headers::DispositionParam;
use iron::headers::DispositionType;
use iron::mime::{Mime, SubLevel, TopLevel};
use iron::modifiers::Redirect;
use iron::prelude::*;
use iron::status;
use iron::Url;

use router::Router;

use handlebars_iron::Template;

use mime_guess;

use persistent;

/// The TextView is used as parameters to the login template.
#[derive(Serialize)]
struct TextView {
    contents: String,
    meta: FileMetadata,
    url: String,
}

/// Helper function which attempts to find a static file enbedded in the executable.
fn get_static_file(filename: &str) -> Option<(&'static [u8], Mime)> {
    let path = Path::new(&filename);

    match get_file(&files, filename) {
        Some(file) => {
            let content_type: Mime = match path.extension() {
                Some(ext) => match ext.to_str() {
                    Some(ext) => match mime_guess::get_mime_type_opt(ext) {
                        Some(v) => v,
                        None => Mime(TopLevel::Application, SubLevel::OctetStream, Vec::new()),
                    },
                    None => Mime(TopLevel::Application, SubLevel::OctetStream, Vec::new()),
                },
                None => Mime(TopLevel::Application, SubLevel::OctetStream, Vec::new()),
            };

            Some((file, content_type))
        }
        _ => None,
    }
}

/// Get pushed file endpoint. Either finds a static file, or failing that, an uploaded file.
///
/// HTTP request required state:
///     Request kind: GET, with filename as part of path
///     Headers: None
pub fn get_pushed_file(req: &mut Request) -> IronResult<Response> {
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
                config.external_url.to_owned()
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
