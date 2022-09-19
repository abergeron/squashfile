#[macro_use]
extern crate static_assertions;

use std::io;
use std::path::Path;
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;

mod disk;
pub mod error;
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

fn extract<P: AsRef<Path>>(dir: &fs::Directory, targ: P) -> Result<()> {
    let target: &Path = targ.as_ref();
    for e in dir.iter() {
        let dent = e?;
        let subp = target.join(OsStr::from_bytes(&dent.file_name()?.as_bytes()));
        match dent.item()? {
            fs::FSItem::File(f) => {
                // This should use a fixed-size buffer and extract by chunks
                let mut buf = vec![0; f.size() as usize];
                f.read_at(buf.as_mut_slice(), 0)?;
                std::fs::write(&subp, buf)?;
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

pub fn extract_image_file<P: AsRef<Path>, T: AsRef<Path>>(image: &P, target: &T) -> Result<()> {
    let fs = open_image_file(image, None, None)?;
    extract(&fs.get_root()?, target)
}

pub fn extract_image<T: AsRef<Path>>(image_data: &[u8], target: &T) -> Result<()> {
    let tmp = image_data.to_vec();
    let fs = fs::FS::open(tmp, None, None)?;
    extract(&fs.get_root()?, target)
}

pub fn open_image_file<P: AsRef<Path>>(
    img: P,
    key: Option<&[u8]>,
    nonce: Option<&[u8]>,
) -> Result<fs::FS> {
    fs::FS::open_file(img, key, nonce)
}
