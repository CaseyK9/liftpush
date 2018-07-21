extern crate includedir_codegen;

use includedir_codegen::Compression;

fn main() {
    includedir_codegen::start("FILES")
        .dir("static", Compression::Gzip)
        .build("data_static.rs")
        .unwrap();

    includedir_codegen::start("TEMPLATES")
        .dir("templates", Compression::Gzip)
        .build("data_templates.rs")
        .unwrap();
}
