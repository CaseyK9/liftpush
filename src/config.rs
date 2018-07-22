//! Contains structures for managing runtime configuration data.

use std::error::Error;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use toml;

use iron::typemap::Key;

/// API key used to programmatically upload files.
#[derive(Deserialize)]
pub struct APIKey {
    pub key: String,
    pub comment: Option<String>,
}

/// A single element of a user's credentials.
#[derive(Deserialize)]
pub struct UserCredentials {
    /// The user's username
    pub username: String,
    /// The user's password, hashed using SHA256
    pub password: String,
}

/// The config file contains configurable runtime properties, as well as user credentials.
#[derive(Deserialize)]
pub struct Config {
    pub bind_addr: String,
    pub external_url: String,
    pub base_path: String,
    pub api_keys: Vec<APIKey>,
    pub users: Vec<UserCredentials>,
    pub key: String,
}

impl Config {
    /// Loads a config file from the specified file.
    pub fn from_file(path: &str) -> Result<Config, String> {
        let path = Path::new(path);
        let mut config_file = match File::open(&path) {
            Ok(file) => file,
            Err(err) => return Err(err.description().to_string()),
        };

        let mut config_contents = String::new();
        match config_file.read_to_string(&mut config_contents) {
            Ok(_) => (),
            Err(err) => return Err(err.description().to_string()),
        }

        match toml::from_str(&config_contents) {
            Ok(config) => Ok(config),
            Err(serde_error) => Err(serde_error.description().to_string()),
        }
    }
}

/// Container used when shipping around the configuration in the web application.
#[derive(Copy, Clone)]
pub struct ConfigContainer;

impl Key for ConfigContainer {
    type Value = Config;
}
