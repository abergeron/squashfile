// Stuff to write images from a folder

use std::io::Seek;
use std::io::Write;
use std::os::unix::ffi::OsStrExt;

use crate::error::Error;
type Result<T> = std::result::Result<T, Error>;
use crate::disk;
use disk::Key;

use std::fs;
use std::io;
use std::path::Path;

trait SeekWrite: Seek + Write {}

impl<T: Seek + Write> SeekWrite for T {}

fn struct_to_slice<T>(ptr: &T) -> &[u8] {
    unsafe { std::slice::from_raw_parts((ptr as *const T) as *const u8, std::mem::size_of::<T>()) }
}

fn write_header<S: SeekWrite>(
    out: &mut S,
    root_inode: u64,
    enc_type: disk::EncryptionType,
) -> Result<()> {
    let mut header = disk::Header::default();
    header.magic = disk::MAGIC;
    header.root_inode = root_inode.into();
    header.version_major = disk::VERSION_MAJOR;
    header.version_minor = disk::VERSION_MINOR;
    header.compression_type = disk::CompressionType::None as u8;
    header.encryption_type = enc_type as u8;
    out.write_all(struct_to_slice(&header))
        .map_err(|e| e.into())
}

fn write_file<P: AsRef<Path>, S: SeekWrite>(file: P, out: &mut S) -> Result<u64> {
    let mut inode = disk::Inode::default();
    inode.offset = out.stream_position()?.into();
    inode.inode_type = disk::InodeType::File.into();
    inode.size = io::copy(&mut fs::File::open(file)?, out)?.into();
    let inode_pos = out.stream_position()?;
    out.write_all(struct_to_slice(&inode))?;
    Ok(inode_pos)
}

fn write_symlink<P: AsRef<Path>, S: SeekWrite>(link: P, out: &mut S) -> Result<u64> {
    let mut inode = disk::Inode::default();
    inode.offset = out.stream_position()?.into();
    inode.inode_type = disk::InodeType::Symlink.into();
    let link_data = fs::read_link(link)?;
    let buf = link_data.as_os_str();
    inode.size = (buf.len() as u64).into();
    out.write_all(buf.as_bytes())?;
    let inode_pos = out.stream_position()?;
    out.write_all(struct_to_slice(&inode))?;
    Ok(inode_pos)
}

fn write_directory<P: AsRef<Path>, S: SeekWrite>(dir: P, out: &mut S) -> Result<u64> {
    let mut entries = Vec::new();
    let iter = fs::read_dir(dir)?;
    let tmp: std::result::Result<Vec<_>, io::Error> = iter.collect();
    let mut paths = tmp?;
    paths.sort_by_key(|e| e.path());
    for entry in paths {
        let ft = entry.file_type()?;
        let name_pos = out.stream_position()?;
        out.write_all(entry.file_name().as_bytes())?;
        out.write(b"\0")?;

        let inode_pos = if ft.is_file() {
            write_file(entry.path(), out)?
        } else if ft.is_symlink() {
            write_symlink(entry.path(), out)?
        } else if ft.is_dir() {
            write_directory(entry.path(), out)?
        } else {
            return Err(Error::InvalidOperation("Unsupported file type"));
        };
        entries.push(disk::Dirent {
            name: name_pos.into(),
            inode: inode_pos.into(),
        })
    }
    let mut dir_inode = disk::Inode::default();
    let buf = unsafe {
        std::slice::from_raw_parts(
            entries.as_ptr() as *const u8,
            entries.len() * std::mem::size_of::<disk::Dirent>(),
        )
    };
    dir_inode.offset = out.stream_position()?.into();
    dir_inode.size = (buf.len() as u64).into();
    dir_inode.inode_type = disk::InodeType::Directory.into();
    out.write_all(buf)?;
    let dir_inode_pos = out.stream_position()?;
    out.write_all(struct_to_slice(&dir_inode))?;
    let inode_ref: disk::u64le = dir_inode_pos.into();

    // Fix up the inodes in the directory to add the correct parent inode
    let cur_pos = out.stream_position()?;
    for dentry in entries {
        out.seek(io::SeekFrom::Start(dentry.inode.into()))?;
        out.write_all(struct_to_slice(&inode_ref))?;
    }
    out.seek(io::SeekFrom::Start(cur_pos))?;

    Ok(dir_inode_pos)
}

pub fn write_image<P: AsRef<Path>, S: Seek + Write>(
    source: P,
    mut out: S,
    key: Key,
    enc_type: disk::EncryptionType,
) -> Result<()> {
    if !fs::metadata(&source)?.is_dir() {
        return Err(Error::InvalidOperation("root is not a directory"));
    }
    // Skip the header for now
    out.seek(io::SeekFrom::Start(
        std::mem::size_of::<disk::Header>() as u64
    ))?;

    // wrap with encrypter eventually
    let mut out_enc: Box<dyn SeekWrite> = match enc_type {
        disk::EncryptionType::None => Box::new(&mut out),
        disk::EncryptionType::ChaCha20 => {
            let enc = disk::crypto::EncryptChaCha20::new(&mut out, key)?;
            Box::new(enc)
        }
    };
    let root_inode = write_directory(&source, &mut out_enc)?;
    // Set the parent of the root inode to itself
    let root_inode_ref: disk::u64le = root_inode.into();
    out_enc.seek(io::SeekFrom::Start(root_inode))?;
    out_enc.write_all(struct_to_slice(&root_inode_ref))?;
    drop(out_enc);

    out.rewind()?;
    write_header(&mut out, root_inode, enc_type)
}
