use std::io::Read;
use std::fs::File;
use std::path::Path;
use std::error::Error;

use serde_json;

#[derive(Deserialize)]
pub struct APIKey {
    pub key : String,
    pub comment : Option<String>
}

#[derive(Deserialize)]
pub struct UserCredentials {
    pub username : String,
    // Sha256
    pub password : String
}

#[derive(Deserialize)]
pub struct Config {
    pub base_url : String,
    pub api_keys : Vec<APIKey>,
    pub users : Vec<UserCredentials>
}

pub fn load_config(path : &str) -> Result<Config, String> {
        let path = Path::new(path);
    let mut config_file = match File::open(&path) {
        Ok(file) => file,
        Err(err) => return Err(err.description().to_string())
    };

    let mut config_contents = String::new();
    match config_file.read_to_string(&mut config_contents) {
        Ok(_) => (),
        Err(err) => return Err(err.description().to_string())
    }

    match serde_json::from_str(&config_contents) {
        Ok(config) => Ok(config),
        Err(serde_error) => Err(serde_error.description().to_string())
    }
}
