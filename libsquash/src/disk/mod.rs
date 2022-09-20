// On disk format, struct and parsing

pub mod write;

#[cfg(test)]
mod tests;

mod crypto;

// This is for read_at/read_exact_at
use std::os::unix::fs::FileExt;

use memchr::memchr;

use crate::error::Error;
use std::convert::TryFrom;
use std::ffi::CString;
use std::io;
use std::cmp::min;

type Result<T> = std::result::Result<T, Error>;

pub static MAGIC: [u8; 8] = *b"SQUASHFL";
pub static VERSION_MAJOR: u8 = 0;
pub static VERSION_MINOR: u8 = 0;

#[derive(Copy, Clone, Debug, Default)]
#[repr(C, packed(8))]
struct u64le {
    val: u64,
}

impl From<u64le> for u64 {
    fn from(v: u64le) -> u64 {
        u64::from_le(v.val)
    }
}

impl From<u64> for u64le {
    fn from(v: u64) -> u64le {
        u64le { val: u64::to_le(v) }
    }
}

#[derive(Copy, Clone, Debug, Default)]
#[repr(C, packed(4))]
struct u32le {
    val: u32,
}

impl From<u32le> for u32 {
    fn from(v: u32le) -> u32 {
        u32::from_le(v.val)
    }
}

impl From<u32> for u32le {
    fn from(v: u32) -> u32le {
        u32le { val: u32::to_le(v) }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum EncryptionType {
    None,
    ChaCha20,
}

impl TryFrom<u8> for EncryptionType {
    type Error = Error;

    fn try_from(val: u8) -> Result<Self> {
        match val {
            0 => Ok(EncryptionType::None),
            1 => Ok(EncryptionType::ChaCha20),
            _ => Err(Error::Format("EncryptionType")),
        }
    }
}

impl From<EncryptionType> for u8 {
    fn from(val: EncryptionType) -> u8 {
        match val {
            EncryptionType::None => 0,
            EncryptionType::ChaCha20 => 1,
        }
    }
}
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum CompressionType {
    None,
}

impl TryFrom<u8> for CompressionType {
    type Error = Error;

    fn try_from(val: u8) -> Result<Self> {
        match val {
            0 => Ok(CompressionType::None),
            _ => Err(Error::Format("CompressionType")),
        }
    }
}

impl From<CompressionType> for u8 {
    fn from(val: CompressionType) -> u8 {
        match val {
            CompressionType::None => 0,
        }
    }
}

#[derive(Copy, Clone, Debug, Default)]
#[repr(C, packed)]
struct Header {
    magic: [u8; 8],
    root_inode: u64le,
    version_major: u8,
    version_minor: u8,
    compression_type: u8,
    encryption_type: u8,
    _pad1: u32,
    _pad2: u64,
}

assert_eq_size!(Header, [u8; 32]);

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum InodeType {
    Directory,
    File,
    Symlink,
}

impl TryFrom<u8> for InodeType {
    type Error = Error;

    fn try_from(val: u8) -> Result<Self> {
        match val {
            0 => Ok(InodeType::Directory),
            1 => Ok(InodeType::File),
            2 => Ok(InodeType::Symlink),
            _ => Err(Error::Format("InodeType")),
        }
    }
}

impl From<InodeType> for u8 {
    fn from(val: InodeType) -> u8 {
        match val {
            InodeType::Directory => 0,
            InodeType::File => 1,
            InodeType::Symlink => 2,
        }
    }
}

#[derive(Copy, Clone, Debug, Default)]
#[repr(C, packed)]
pub struct Inode {
    parent_inode: u64le,
    offset: u64le,
    size: u64le,
    inode_type: u8,
    _pad: [u8; 7],
}

assert_eq_size!(Inode, [u8; 32]);

#[derive(Copy, Clone, Debug, Default)]
#[repr(C, packed)]
pub struct Dirent {
    name: u64le,
    inode: u64le,
}

assert_eq_size!(Dirent, [u8; 16]);

pub struct Image {
    file: Box<dyn ReadAt>,
    header: Header,
    // we only have None for the compression and encryption for now
    // later there will be fields here to deal with those
}

fn struct_to_mut_slice<T>(ptr: &mut T) -> &mut [u8] {
    unsafe { std::slice::from_raw_parts_mut((ptr as *mut T) as *mut u8, std::mem::size_of::<T>()) }
}

fn read_header<T: ReadAt>(file: &T) -> Result<Header> {
    let mut buf = Header::default();
    file.read_exact_at(struct_to_mut_slice(&mut buf), 0)?;
    Ok(buf)
}

pub trait ReadAt {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> Result<usize>;

    fn read_exact_at(&self, mut buf: &mut [u8], mut offset: u64) -> Result<()> {
        while !buf.is_empty() {
            match self.read_at(buf, offset) {
                Ok(0) => break,
                Ok(n) => {
                    let tmp = buf;
                    buf = &mut tmp[n..];
                    offset += n as u64;
                }
                Err(Error::IO(ref e)) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
        if !buf.is_empty() {
            Err(Error::IO(io::Error::from(io::ErrorKind::UnexpectedEof)))
        } else {
            Ok(())
        }
    }
}

impl ReadAt for std::fs::File
{
    fn read_at(&self, buf: &mut [u8], offset: u64) -> Result<usize> {
        Ok(FileExt::read_at(self, buf, offset)?)
    }
}

impl ReadAt for Vec<u8> {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> Result<usize> {
        let s = self.as_slice();
        if offset > s.len() as u64 {
            return Ok(0);
        }
        let sz = min(buf.len(), s.len() - offset as usize);
        let off = offset as usize;
        buf[..sz].copy_from_slice(&s[off..off+sz]);
        Ok(sz)
    }
}

pub fn open_file<F: ReadAt + 'static>(
    file: F,
    key: Option<&[u8]>,
    nonce: Option<&[u8]>,
) -> Result<Image> {
    let header = read_header(&file)?;

    if header.magic != MAGIC {
        return Err(Error::Format("Wrong magic"));
    }

    if header.version_major != VERSION_MAJOR {
        return Err(Error::Format("Unsupported major version"));
    }

    if header.version_minor != VERSION_MINOR {
        return Err(Error::Format("Unsupported minor version"));
    }

    let stream: Box<dyn ReadAt> = match EncryptionType::try_from(header.encryption_type)? {
        EncryptionType::None => Box::new(file),
        EncryptionType::ChaCha20 => {
            if let Some(key) = key {
                if let Some(nonce) = nonce {
                    Box::new(crypto::EncryptChaCha20::new(file, key, nonce)?)
                } else {
                    return Err(Error::Crypto("No provided nonce"));
                }
            } else {
                return Err(Error::Crypto("No provided key"));
            }
        }
    };

    let comp_type = CompressionType::try_from(header.compression_type)?;
    if comp_type != CompressionType::None {
        return Err(Error::Bounds("Unsupported compression type"));
    }

    Ok(Image {
        file: stream,
        header: header,
    })
}

impl Header {
    pub fn root_inode(&self, img: &Image) -> Result<Inode> {
        img.read_inode(self.root_inode.into())
    }
}

impl Inode {
    pub fn parent_inode(&self, img: &Image) -> Result<Inode> {
        img.read_inode(self.parent_inode.into())
    }

    pub fn inode_type(&self) -> Result<InodeType> {
        InodeType::try_from(self.inode_type)
    }

    pub fn read_dirent(&self, pos: u64, img: &Image) -> Result<Dirent> {
        if self.inode_type()? != InodeType::Directory {
            return Err(Error::InvalidOperation(
                "Reading dirents from non-directory",
            ));
        }
        let offset = pos * std::mem::size_of::<Dirent>() as u64;
        if offset > self.size() {
            return Err(Error::Bounds("dirent pos is beyond the directory"));
        }
        img.read_dirent(offset + u64::from(self.offset))
    }

    pub fn read_at(&self, buf: &mut [u8], off: u64, img: &Image) -> Result<usize> {
        if off > self.size() {
            return Ok(0);
        }
        let sz = min(buf.len() as u64, self.size() - off) as usize;
        img.read_file(&mut buf[..sz], u64::from(self.offset) + off)?;
        Ok(sz)
    }

    pub fn read_exact_at(&self, buf: &mut [u8], off: u64, img: &Image) -> Result<()> {
        if off + buf.len() as u64 > self.size() {
            Err(io::Error::from(io::ErrorKind::UnexpectedEof).into())
        } else {
            img.read_file(buf, u64::from(self.offset) + off)
        }
    }

    pub fn size(&self) -> u64 {
        self.size.into()
    }
}

impl Dirent {
    pub fn inode(&self, img: &Image) -> Result<Inode> {
        img.read_inode(self.inode.into())
    }

    pub fn name(&self, img: &Image) -> Result<CString> {
        img.read_str(self.name.into())
    }
}

impl Image {
    fn read_inode(&self, off: u64) -> Result<Inode> {
        let mut buf = Inode::default();
        self.file
            .read_exact_at(struct_to_mut_slice(&mut buf), off)?;
        Ok(buf)
    }

    fn read_dirent(&self, off: u64) -> Result<Dirent> {
        let mut buf = Dirent::default();
        self.file
            .read_exact_at(struct_to_mut_slice(&mut buf), off)?;
        Ok(buf)
    }

    fn read_str(&self, off: u64) -> Result<CString> {
        let mut buf = Vec::new();
        let mut off = off;
        let mut tmp_read = [0; 32];

        loop {
            let read = self.file.read_at(&mut tmp_read, off)?;
            if read == 0 {
                return Err(Error::IO(io::Error::from(io::ErrorKind::UnexpectedEof)));
            }
            // In case of a short read
            let tmp = &tmp_read[..read];
            off += read as u64;
            match memchr(0, &tmp) {
                Some(i) => {
                    buf.extend_from_slice(&tmp[..=i]);
                    return Ok(unsafe { CString::from_vec_with_nul_unchecked(buf) });
                }
                None => buf.extend_from_slice(tmp),
            }
        }
    }

    fn read_file(&self, buf: &mut [u8], off: u64) -> Result<()> {
        self.file.read_exact_at(buf, off).map_err(|e| e.into())
    }

    pub fn root_inode(&self) -> Result<Inode> {
        self.header.root_inode(self)
    }
}
