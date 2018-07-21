//! Generic helper types.

use std::error::Error;
use std::fmt::{self, Debug, Display, Formatter};

/// Used for representing generic String errors as IronErrors.
#[derive(Debug)]
pub struct StringError(pub String);

impl Error for StringError {}

impl Display for StringError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Debug::fmt(self, f)
    }
}
