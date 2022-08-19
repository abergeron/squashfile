// On disk format, struct and parsing

pub mod write;

// This is for read_at/read_exact_at
use std::os::unix::fs::FileExt;

use memchr::memchr;

use crate::error::Error;
use std::convert::TryFrom;
use std::ffi::CString;
use std::fs;
use std::io;

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
}

impl TryFrom<u8> for EncryptionType {
    type Error = Error;

    fn try_from(val: u8) -> Result<Self> {
        match val {
            0 => Ok(EncryptionType::None),
            _ => Err(Error::Format("EncryptionType".into())),
        }
    }
}

impl From<EncryptionType> for u8 {
    fn from(val: EncryptionType) -> u8 {
        match val {
            EncryptionType::None => 0,
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
            _ => Err(Error::Format("CompressionType".into())),
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
    encryption_data_offset: u32le,
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
            _ => Err(Error::Format("InodeType".into())),
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

#[derive(Debug)]
pub struct Image {
    file: fs::File,
    header: Header,
    // we only have None for the compression and encryption for now
    // later there will be fields here to deal with those
}

fn struct_to_mut_slice<T>(ptr: &mut T) -> &mut [u8] {
    unsafe { std::slice::from_raw_parts_mut((ptr as *mut T) as *mut u8, std::mem::size_of::<T>()) }
}

fn read_header(file: &fs::File) -> Result<Header> {
    let mut buf = Header::default();
    file.read_exact_at(struct_to_mut_slice(&mut buf), 0)?;
    Ok(buf)
}

pub fn open_file<P: AsRef<std::path::Path>>(path: P) -> Result<Image> {
    let file = fs::File::open(path)?;

    let header = read_header(&file)?;

    if header.magic != MAGIC {
        return Err(Error::Format("Wrong magic".into()));
    }

    if header.version_major != VERSION_MAJOR {
        return Err(Error::Format("Unsupported major version".into()));
    }

    if header.version_minor != VERSION_MINOR {
        return Err(Error::Format("Unsupported minor version".into()));
    }

    let comp_type = CompressionType::try_from(header.compression_type)?;
    if comp_type != CompressionType::None {
        return Err(Error::Bounds("Unsupported compression type".into()));
    }

    if EncryptionType::try_from(header.encryption_type)? != EncryptionType::None {
        return Err(Error::Bounds("Unsupported encryption type".into()));
    }

    Ok(Image {
        file: file,
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
                "Reading dirents from non-directory".into(),
            ));
        }
        let offset = u64::from(self.offset) + (pos * std::mem::size_of::<Dirent>() as u64);
        if offset > self.size() {
            return Err(Error::Bounds("dirent pos is beyond the directory".into()));
        }
        img.read_dirent(offset)
    }

    pub fn read_at(&self, buf: &mut [u8], off: u64, img: &Image) -> Result<()> {
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
            // XXX: will this infinite loop on EOF?
            let read = self.file.read_at(&mut tmp_read, off)?;
            // In case of a short read
            let tmp = &tmp_read[..=read];
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
