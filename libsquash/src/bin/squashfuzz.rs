#[macro_use]
extern crate afl;

use libsquash::extract_image;

use std::fs::{create_dir, remove_dir_all};

fn main() {
    fuzz!(|data: &[u8]| {
        let _ = remove_dir_all(String::from("/tmp/squashfuzz"));
        let _ = create_dir(String::from("/tmp/squashfuzz"));
        let _ = extract_image(data, &String::from("/tmp/squashfuzz"));
    })
}
