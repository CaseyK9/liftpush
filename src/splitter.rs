//! Middleware used for splitting between authenticated/non-authenticated requests.

use iron::prelude::*;
use iron::Handler;

/// Splits a request depending on the start of a URL. Used for splitting up
/// authenticated/non-authenticated endpoints.
pub struct ChainSplit<A: Handler, B: Handler> {
    left: A,
    right: B,
    split_on: Vec<String>,
    equal_to: Vec<String>,
}

impl<A: Handler, B: Handler> ChainSplit<A, B> {
    /// Creates a new split handler.
    ///
    /// split_on: If the URL starts with this, go left. No leading slash.
    /// equal_to: If the URL is equal to this, go left. No leading slash.
    pub fn new(
        left: A,
        right: B,
        split_on: Vec<String>,
        equal_to: Vec<String>,
    ) -> ChainSplit<A, B> {
        ChainSplit {
            left,
            right,
            split_on,
            equal_to,
        }
    }
}

impl<A: Handler, B: Handler> Handler for ChainSplit<A, B> {
    fn handle(&self, request: &mut Request) -> IronResult<Response> {
        let path = request.url.path().join("/");

        let mut found = false;
        for elem in &self.split_on {
            if path.starts_with(elem) {
                found = true;
                break;
            }
        }

        for elem in &self.equal_to {
            if &path == elem {
                found = true;
                break;
            }
        }

        if found {
            self.left.handle(request)
        } else {
            self.right.handle(request)
        }
    }
}
