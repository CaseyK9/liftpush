use rocket::Outcome::*;
use rocket::State;
use rocket::request::{Request, Outcome, FromRequest};
use rocket::http::Status;

use rand;
use rand::Rng;

use std::iter::FromIterator;

fn as_capital_case(input : &str) -> String {
    let mut result = String::new();

    if input.len() == 0 {
        return result;
    }

    result += &(&input[0..1]).to_uppercase();

    if input.len() == 1 {
        return result;
    }

    result += &input[1..];
    result
}

pub struct PhraseGenerator {
    adjectives : Vec<String>,
    nouns : Vec<String>
}

impl PhraseGenerator {
    fn generate(&self) -> String {
        let mut rng = rand::thread_rng();

        let mut result = String::new();

        // TODO: Custom length
        for _ in 0 .. 1 {
            let adjectives_ptr = rng.gen_range(0, self.adjectives.len());
            result += &as_capital_case(&self.adjectives[adjectives_ptr]);
        }

        let nouns_ptr = rng.gen_range(0, self.nouns.len());
        result += &as_capital_case(&self.nouns[nouns_ptr]);

        result
    }

    pub fn new(adjectives : &str, nouns : &str) -> Self {
        let adjectives : Vec<String> = Vec::from_iter(adjectives.split("\n").map(String::from));
        let nouns : Vec<String> = Vec::from_iter(nouns.split("\n").map(String::from));

        Self {
            adjectives, nouns
        }
    }
}

pub struct RandomFilename {
    pub filename : String
}

impl<'a, 'r> FromRequest<'a, 'r> for RandomFilename {
    type Error = ();

    fn from_request(request: &'a Request<'r>) -> Outcome<Self, Self::Error> {
        let phrases : State<PhraseGenerator> = match request.guard::<State<PhraseGenerator>>() {
            Success(phrases) => phrases,
            _ => return Failure((Status::ServiceUnavailable, ()))
        };

        Success(RandomFilename {
            filename : phrases.generate()
        })
    }
}

pub struct MultipartBoundary {
    pub boundary : String
}

impl<'a, 'r> FromRequest<'a, 'r> for MultipartBoundary {
    type Error = ();

    fn from_request(request: &'a Request<'r>) -> Outcome<Self, Self::Error> {
        let ct = match request.headers().get_one("Content-Type") {
            Some(val) => val,
            None => return Failure((Status::BadRequest, ()))
        };

        let idx = match ct.find("boundary=") {
            Some(val) => val,
            None => return Failure((Status::BadRequest, ()))
        };

        let boundary = ct[(idx + "boundary=".len())..].to_owned();

        Success(MultipartBoundary {
            boundary
        })
    }
}
