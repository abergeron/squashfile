#[macro_use]
extern crate static_assertions;

use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::io::Cursor;

mod disk;
pub mod error;
pub mod fs;

pub use disk::{CompressionType, EncryptionType, Key};
pub use error::Error;
pub type Result<T> = std::result::Result<T, Error>;

pub use disk::write::write_image;

pub fn write_image_file<P: AsRef<Path>, S: AsRef<Path>>(
    source: &P,
    file: &S,
    key: Key,
    enc_type: EncryptionType,
) -> Result<()> {
    let mut file = std::fs::File::create(file)?;
    write_image(source, &mut file, key, enc_type)
}

fn extract<P: AsRef<Path>>(dir: &fs::Directory, targ: P) -> Result<()> {
    let target: &Path = targ.as_ref();
    for e in dir.iter() {
        let dent = e?;
        let subp = target.join(OsStr::from_bytes(&dent.file_name()?.as_bytes()));
        match dent.item()? {
            fs::FSItem::File(ref mut f) => {
                let mut t = std::fs::File::create(&subp)?;
                std::io::copy(f, &mut t)?;
            }
            fs::FSItem::Directory(d) => {
                std::fs::create_dir(&subp)?;
                extract(&d, &subp)?;
            }
            fs::FSItem::Symlink(s) => {
                std::os::unix::fs::symlink(OsStr::from_bytes(s.get_link()?.as_slice()), &subp)?;
            }
        }
    }
    Ok(())
}

pub fn extract_image_file<P: AsRef<Path>, T: AsRef<Path>>(
    image: &P,
    target: &T,
    key: Key,
) -> Result<()> {
    let fs = open_image_file(image, key)?;
    extract(&fs.get_root()?, target)
}

pub fn extract_image<T: AsRef<Path>>(
    image_data: &[u8],
    target: &T,
    key: Key,
) -> Result<()> {
    let tmp = Cursor::new(image_data.to_vec());
    let fs = fs::FS::open(tmp, key)?;
    extract(&fs.get_root()?, target)
}

pub fn open_image_file<P: AsRef<Path>>(img: P, key: Key) -> Result<fs::FS> {
    fs::FS::open_file(img, key)
}
