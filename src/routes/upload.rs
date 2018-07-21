//! Hosts the upload API endpoint.

use ConfigContainer;

use types::FileMetadata;
use types::FileType;
use types::StringError;

use io::RandomFilename;

use std::fs::copy;
use std::fs::File;
use std::io::Read;
use std::io::Write;
use std::path::Path;

use iron::mime::{Mime, SubLevel, TopLevel};
use iron::prelude::*;
use iron::status;
use iron::Error;

use params::Params;
use params::Value;

use persistent;

use router::Router;

use serde_json;

/// Used to build the JSON payload as a response to an upload.
#[derive(Serialize)]
struct UploadStatus {
    url: String,
}

/// Upload endpoint. Uploads a specified file for a user.
///
/// HTTP request required state:
///     Request kind: POST, with a embedded file as "pushfile"
///     Headers: required X-API-Key
pub fn upload(req: &mut Request) -> IronResult<Response> {
    let base_path = {
        let arc = req
            .get::<persistent::Read<ConfigContainer>>()
            .expect("No ConfigContainer object available");
        let config = arc.as_ref();
        config.base_path.to_owned()
    };

    // Verify API key
    let api_key = {
        let raw_key = req.headers.get_raw("X-API-Key").ok_or_else(|| {
            IronError::new(
                StringError("Unable to find API key in submitted form".into()),
                (status::BadRequest, "Missing API key"),
            )
        })?;

        String::from_utf8(raw_key[0].to_owned())
            .map_err(|x| IronError::new(x, (status::BadRequest, "Internal I/O error")))?
    };

    {
        let arc = req
            .get::<persistent::Read<ConfigContainer>>()
            .expect("No ConfigContainer object available");
        let config = arc.as_ref();

        // Find target user
        let mut found = false;

        for key in &config.api_keys {
            if key.key == api_key {
                found = true;
            }
        }

        if !found {
            return Err(IronError::new(
                StringError("Bad API key in submitted form".into()),
                (status::BadRequest, "Bad API key"),
            ));
        }
    }

    // Fetch other request attributes
    let file = {
        let map = req.get_ref::<Params>().expect("No Params object available");

        extract_param_type!(map, File, "pushfile")?.to_owned()
    };

    let input_type = {
        req.extensions
            .get::<Router>()
            .expect("No Router object available")
            .find("type")
            .ok_or_else(|| {
                IronError::new(
                    StringError("No input type specified".into()),
                    (status::BadRequest, "No input type specified"),
                )
            })?
            .to_owned()
    };

    let input_type = match FileType::from_str(&input_type) {
        Some(val) => val,
        _ => {
            return Err(IronError::new(
                StringError("Invalid input type specified".into()),
                (status::BadRequest, "Invalid input type"),
            ))
        }
    };

    let out_file = RandomFilename::from(req)?;

    // Generate metadata properties
    let original_filename = match file.filename.clone() {
        Some(filename) => filename,
        _ => {
            return Err(IronError::new(
                StringError("No multipart filename specified".into()),
                (status::BadRequest, "No multipart filename specified"),
            ))
        }
    };

    let ext_split = original_filename.clone();
    let ext: Option<&str> = ext_split.split(".").last();

    let url = out_file.filename.clone();

    let new_filename = match ext {
        Some(ext) => out_file.filename + "." + ext,
        _ => out_file.filename,
    };

    let base_url = {
        let arc = req
            .get::<persistent::Read<ConfigContainer>>()
            .expect("Config file is missing");
        let config = arc.as_ref();
        config.external_url.to_owned()
    };

    println!(
        "Saving {:?} of type {:?} to {:?}",
        original_filename, input_type, new_filename
    );

    // Generate metadata
    let meta = match input_type {
        // General file upload
        FileType::File => {
            let target_file = Path::new(&base_path).join(&new_filename);

            if target_file.exists() {
                return Err(IronError::new(
                    StringError(format!("Target file {:?} already exists", target_file)),
                    (status::BadRequest, "Internal I/O error"),
                ));
            }

            copy(file.path, target_file)
                .map_err(|x| IronError::new(x, (status::BadRequest, "Internal I/O error")))?;
            FileMetadata::new_from_file(original_filename, new_filename.clone())
        }
        // Specifc text file upload
        FileType::Text => {
            let mut data = Vec::new();
            file.open()
                .map_err(|x| IronError::new(x, (status::BadRequest, "Internal I/O error")))?
                .read_to_end(&mut data)
                .map_err(|x| IronError::new(x, (status::BadRequest, "Internal I/O error")))?;

            let mut data = String::from_utf8(data)
                .map_err(|x| IronError::new(x, (status::BadRequest, "Internal I/O error")))?;

            if data[0..8].contains("://") {
                FileMetadata::new_from_url(data)
            } else {
                // Save buffered text
                let meta_filename = base_path.to_string() + &new_filename;
                let path = Path::new(&meta_filename);

                let mut meta_file = match File::create(&path) {
                    Err(why) => {
                        println!("Couldn't create {}: {}", meta_filename, why.description());
                        return Err(IronError::new(
                            StringError("Failed to write file".into()),
                            (status::InternalServerError, "Failed to write file"),
                        ));
                    }
                    Ok(file) => file,
                };

                match meta_file.write_all(data.as_bytes()) {
                    Err(why) => {
                        println!(
                            "Failed to write to {}: {}",
                            meta_filename,
                            why.description()
                        );
                        return Err(IronError::new(
                            StringError("Failed to write file".into()),
                            (status::InternalServerError, "Failed to write file"),
                        ));
                    }
                    Ok(_) => (),
                }

                FileMetadata::new_from_text(original_filename, new_filename.clone())
            }
        }
        _ => {
            // URL
            return Err(IronError::new(
                StringError("URL uploading type not supported".into()),
                (status::InternalServerError, "URL type not implemented"),
            ));
        }
    };

    let meta_string = match serde_json::to_string(&meta) {
        Ok(data) => data,
        Err(msg) => {
            println!("Couldn't serialize metadata: {}", msg.description());
            return Err(IronError::new(
                StringError("Failed to generate metadata".into()),
                (status::InternalServerError, "Failed to generate metadata"),
            ));
        }
    };

    // Save metadata
    let meta_filename = base_path.to_string() + &url + ".info.json";
    let path = Path::new(&meta_filename);

    let mut meta_file = match File::create(&path) {
        Err(why) => {
            println!("Couldn't create {}: {}", meta_filename, why.description());
            return Err(IronError::new(
                StringError("Failed to write file".into()),
                (status::InternalServerError, "Failed to write file"),
            ));
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
            return Err(IronError::new(
                StringError("Failed to write file".into()),
                (status::InternalServerError, "Failed to write file"),
            ));
        }
        Ok(_) => (),
    }

    let response = serde_json::to_string(&UploadStatus {
        url: base_url + &url,
    }).map_err(|x| IronError::new(x, (status::BadRequest, "Internal I/O error")))?;

    Ok(Response::with((
        status::Ok,
        response,
        Mime(TopLevel::Application, SubLevel::Json, Vec::new()),
    )))
}
