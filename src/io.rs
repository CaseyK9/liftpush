use iron::prelude::*;

use persistent;

use rand;
use rand::Rng;

use iron::typemap::Key;
use std::iter::FromIterator;

fn as_capital_case(input: &str) -> String {
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
    adjectives: Vec<String>,
    nouns: Vec<String>,
}

impl PhraseGenerator {
    fn generate(&self) -> String {
        let mut rng = rand::thread_rng();

        let mut result = String::new();

        // TODO: Custom length
        for _ in 0..1 {
            let adjectives_ptr = rng.gen_range(0, self.adjectives.len());
            result += &as_capital_case(&self.adjectives[adjectives_ptr]);
        }

        let nouns_ptr = rng.gen_range(0, self.nouns.len());
        result += &as_capital_case(&self.nouns[nouns_ptr]);

        result
    }

    pub fn new(adjectives: &str, nouns: &str) -> Self {
        let adjectives: Vec<String> = Vec::from_iter(adjectives.split("\n").map(String::from));
        let nouns: Vec<String> = Vec::from_iter(nouns.split("\n").map(String::from));

        Self { adjectives, nouns }
    }
}

#[derive(Copy, Clone)]
pub struct PhraseGeneratorContainer;

impl Key for PhraseGeneratorContainer {
    type Value = PhraseGenerator;
}

pub struct RandomFilename {
    pub filename: String,
}

impl RandomFilename {
    pub fn from(req: &mut Request) -> IronResult<RandomFilename> {
        let arc = req
            .get::<persistent::Read<PhraseGeneratorContainer>>()
            .unwrap();
        let phrases = arc.as_ref();

        Ok(RandomFilename {
            filename: phrases.generate(),
        })
    }
}
