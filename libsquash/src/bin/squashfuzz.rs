#[macro_use]
extern crate afl;

use libsquash::extract_image;

fn main() {
    fuzz!(|data: &[u8]| {
        let _ = extract_image(data, &String::from("/tmp"));
    })
}
