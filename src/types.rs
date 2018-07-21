//! Generic helper types.

use std::error::Error;
use std::fmt::{self, Debug, Display, Formatter};

use iron::IronError;

use chrono::DateTime;
use chrono::FixedOffset;

/// Used for representing generic String errors as IronErrors.
#[derive(Debug)]
pub struct StringError(pub String);

impl Error for StringError {}

impl Display for StringError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Debug::fmt(self, f)
    }
}

/// The different file types supported.
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub enum FileType {
    File,
    Url,
    Text,
}

impl FileType {
    /// Parses a file type from a String form.
    pub fn from_str(name: &str) -> Option<FileType> {
        match name {
            "file" => Some(FileType::File),
            "url" => Some(FileType::Url),
            "text" => Some(FileType::Text),
            _ => None,
        }
    }
}

/// Implements a RFC2822 serializer for serde (compatible with the original PHP implementation)
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

/// The main metadata store for files.
#[derive(Serialize, Deserialize)]
pub struct FileMetadata {
    /// Date when this file was uploaded.
    #[serde(with = "metadata_rfc2822")]
    pub date: DateTime<FixedOffset>, // Mon, 11 Dec 2017 10:28:36 +0000"

    /// What kind of file this is.
    #[serde(rename = "type")]
    pub file_type: FileType,

    /// Specifies a URL to redirect to.
    /// If this is a URL filetype, this is required.
    pub url: Option<String>,

    /// The filename in which this exists on the filesystem.
    /// If this is a file filetype, this is required.
    pub filename: Option<String>,

    /// The original filename the user specified.
    /// If this is a file filetype, this is required.
    pub actual_filename: Option<String>,
}
