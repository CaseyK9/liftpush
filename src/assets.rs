//! Contains baked-in assets.

use phf;

macro_rules! include_files_as_assets {
    ( $field_name:ident, $file_base:expr, $( $file_name:expr ),* ) => {
        pub static $field_name: phf::Map<&'static str, &'static [u8]> = phf_map!(
            $(
                $file_name => include_bytes!(concat!($file_base, $file_name)),
            )*
        );
    }
}

include_files_as_assets!(
    FILES,
    "../static/",
    "css/agate.css",
    "css/bulma.min.css",
    "css/main.css",
    "js/highlight.pack.js",
    "js/manage.js",
    "js/vue.min.js"
);

include_files_as_assets!(
    TEMPLATES,
    "../templates/",
    "footer.hbs",
    "header.hbs",
    "index.hbs",
    "manage.hbs",
    "text.hbs"
);

/// Returns the contents of a file from the given list.
pub fn get_file(
    table: &phf::Map<&'static str, &'static [u8]>,
    name: &str,
) -> Option<&'static [u8]> {
    table.get(name).map(|x| *x)
}

/// Returns the contents of a file from the given list.
pub fn list_files(table: &phf::Map<&'static str, &'static [u8]>) -> Vec<&'static str> {
    table.keys().map(|x| *x).collect()
}
