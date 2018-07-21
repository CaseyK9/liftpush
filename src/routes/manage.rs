//! Contains the management endpoint + types.

use config::ConfigContainer;

use auth::SessionStore;

use types::FileMetadata;
use types::StringError;

use std::fs;
use std::fs::DirEntry;

use iron::prelude::*;
use iron::status;

use handlebars_iron::Template;

use persistent;

use serde_json;

/// Metadata for each file.
#[derive(Serialize)]
struct ManageMetadata {
    name: String,
    meta: FileMetadata,
}

/// Metadata for all files.
#[derive(Serialize)]
pub struct FileViewerState {
    username: String,
}

/// Files sent when a user requests them.
#[derive(Serialize)]
pub struct FileListing {
    files: Vec<ManageMetadata>,
}

/// Manage endpoint. Spawns a user interface for interactive management of files.
///
/// HTTP request required state:
///     Request kind: GET
///     Headers: required SessionStore
pub fn manage(req: &mut Request) -> IronResult<Response> {
    let user = req.extensions.get::<SessionStore>().ok_or_else(|| {
        IronError::new(
            StringError("User attempted to access restricted page".into()),
            (status::Unauthorized, "You are not logged in"),
        )
    })?;

    Ok(Response::with((
        status::Ok,
        Template::new(
            "manage",
            &FileViewerState {
                username: user.username.to_owned(),
            },
        ),
    )))
}

pub fn listing(req: &mut Request) -> IronResult<Response> {
    let base_path = {
        let arc = req.get::<persistent::Read<ConfigContainer>>().unwrap();
        let config = arc.as_ref();
        config.base_path.to_owned()
    };

    req.extensions.get::<SessionStore>().ok_or_else(|| {
        IronError::new(
            StringError("User attempted to access restricted page".into()),
            (status::Unauthorized, "You are not logged in"),
        )
    })?;

    let paths = fs::read_dir(&base_path).unwrap();

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
        serde_json::to_string(&FileListing {
                files: found_files
            }).map_err(|x| IronError::new(x, (status::BadRequest, "Internal I/O error")))?,
    )))
}
