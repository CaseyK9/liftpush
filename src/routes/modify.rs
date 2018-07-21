//! Endpoints for interactive modification of files.

use config::ConfigContainer;

use types::FileMetadata;
use types::StringError;

use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;

use iron::prelude::*;
use iron::status;
use iron::Error;

use router::Router;

use persistent;

use serde_json;

/// Delete endpoint. Deletes the specified file from the filesystem + metadata.
///
/// HTTP request required state:
///     Request kind: GET, with filename as part of path
///     Headers: required SessionStore
pub fn delete_file(req: &mut Request) -> IronResult<Response> {
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

/// Rename endpoint. Moves the specified file in the filesystem + metadata to a new filename.
///
/// HTTP request required state:
///     Request kind: GET, with source + target as part of path
///     Headers: required SessionStore
pub fn rename_file(req: &mut Request) -> IronResult<Response> {
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
        return Err(IronError::new(
            StringError(format!("Source path {:?} contains bad characters", file)),
            status::NotFound,
        ));
    }

    let mut meta = FileMetadata::from_path(&base_path, &file).map_err(|x| {
        IronError::new(
            StringError(x),
            (status::NotFound, "Failed to find metadata"),
        )
    })?;

    if to.contains(".") || to.contains("/") || to.contains("\\") {
        return Err(IronError::new(
            StringError(format!("Target path {:?} contains bad characters", to)),
            status::NotFound,
        ));
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
