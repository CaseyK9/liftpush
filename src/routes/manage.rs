//! Contains the management endpoint + types.

use ConfigContainer;

use auth::SessionStore;

use types::FileMetadata;
use types::StringError;

use std::fs;
use std::fs::DirEntry;

use iron::prelude::*;
use iron::status;

use handlebars_iron::Template;

use persistent;

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
    files: Vec<ManageMetadata>,
}

/// Manage endpoint. Spawns a user interface for interactive management of files.
///
/// HTTP request required state:
///     Request kind: GET
///     Headers: required SessionStore
pub fn manage(req: &mut Request) -> IronResult<Response> {
    let base_path = {
        let arc = req.get::<persistent::Read<ConfigContainer>>().unwrap();
        let config = arc.as_ref();
        config.base_path.to_owned()
    };

    let user = req.extensions.get::<SessionStore>().ok_or_else(|| {
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
