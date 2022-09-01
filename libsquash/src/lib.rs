#[macro_use]
extern crate static_assertions;

use std::io;
use std::path::Path;

mod disk;
mod error;
pub mod fs;

use error::Error;
pub type Result<T> = std::result::Result<T, Error>;

pub fn write_image<P: AsRef<Path>, S: io::Write + io::Seek>(source: &P, out: &mut S) -> Result<()> {
    disk::write::write_image(source, out)
}

pub fn write_image_file<P: AsRef<Path>, S: AsRef<Path>>(source: &P, file: &S) -> Result<()> {
    let mut file = std::fs::File::create(file)?;
    write_image(source, &mut file)
}

pub fn open_image_file<P: AsRef<Path>>(img: P) -> Result<fs::FS> {
    fs::FS::open(img)
}
