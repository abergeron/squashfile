// std::fs-like interface (read-only of course)

use crate::disk;
use crate::error::Error;

use std::cmp::Ordering;
use std::ffi::CString;
use std::iter::Iterator;
use std::path;
use std::sync::Arc;

type Result<T> = std::result::Result<T, Error>;

// This is relatively low because we deal with it by recursion and
// I don't want to blow the stack.
const LINK_LOOP_MAX: u16 = 100;
// Max length of a symlink target
const LINK_TARGET_MAX: usize = 1024;


#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct FileType {
    ty: disk::InodeType,
}

#[derive(Clone)]
pub struct DirEntry {
    img: Arc<disk::Image>,
    ent: disk::Dirent,
}

#[derive(Clone)]
pub struct Directory {
    img: Arc<disk::Image>,
    inode: disk::Inode,
}

#[derive(Clone)]
pub struct File {
    img: Arc<disk::Image>,
    inode: disk::Inode,
}

#[derive(Clone)]
pub struct Symlink {
    img: Arc<disk::Image>,
    inode: disk::Inode,
}

pub enum FSItem {
    File(File),
    Directory(Directory),
    Symlink(Symlink),
}

fn new_fsitem(img: Arc<disk::Image>, inode: disk::Inode) -> Result<FSItem> {
    Ok(match inode.inode_type()? {
        disk::InodeType::File => FSItem::File(File::new(inode, img)),
        disk::InodeType::Directory => FSItem::Directory(Directory::new(inode, img)),
        disk::InodeType::Symlink => FSItem::Symlink(Symlink::new(inode, img)),
    })
}

pub struct ReadDir {
    dir: Directory,
    pos: u64,
}

pub struct FS {
    img: Arc<disk::Image>,
}

impl FileType {
    pub fn is_dir(&self) -> bool {
        self.ty == disk::InodeType::Directory
    }
    pub fn is_file(&self) -> bool {
        self.ty == disk::InodeType::File
    }
    pub fn is_symlink(&self) -> bool {
        self.ty == disk::InodeType::Symlink
    }
}

impl DirEntry {
    pub fn file_type(&self) -> Result<FileType> {
        Ok(FileType {
            ty: self.ent.inode(self.img.as_ref())?.inode_type()?,
        })
    }

    pub fn file_name(&self) -> Result<CString> {
        self.ent.name(self.img.as_ref())
    }

    pub fn item(&self) -> Result<FSItem> {
        let inode = self.ent.inode(self.img.as_ref())?;
        Ok(new_fsitem(self.img.clone(), inode)?)
    }
}

impl Directory {
    fn new(inode: disk::Inode, img: Arc<disk::Image>) -> Self {
        std::debug_assert!(inode.inode_type().expect("") == disk::InodeType::Directory);
        Directory {
            inode: inode,
            img: img,
        }
    }

    pub fn len(&self) -> u64 {
        self.inode.size() / std::mem::size_of::<disk::Dirent>() as u64
    }

    pub fn resolve<P: AsRef<[u8]>>(&self, path: P) -> Result<Option<FSItem>> {
        resolve_dir(self.img.clone(), self, path)
    }

    pub fn get(&self, pos: u64) -> Result<Option<DirEntry>> {
        if pos >= self.len() {
            Ok(None)
        } else {
            Ok(Some(DirEntry {
                ent: self.inode.read_dirent(pos, self.img.as_ref())?,
                img: self.img.clone(),
            }))
        }
    }

    pub fn iter(&self) -> ReadDir {
        ReadDir {
            dir: self.clone(),
            pos: 0,
        }
    }
}

impl File {
    fn new(inode: disk::Inode, img: Arc<disk::Image>) -> Self {
        std::debug_assert!(inode.inode_type().expect("") == disk::InodeType::File);
        File {
            inode: inode,
            img: img,
        }
    }

    pub fn size(&self) -> u64 {
        self.inode.size()
    }

    pub fn read_at(&self, buf: &mut [u8], offset: u64) -> Result<()> {
        self.inode.read_at(buf, offset, self.img.as_ref())
    }
}

impl Symlink {
    fn new(inode: disk::Inode, img: Arc<disk::Image>) -> Self {
        std::debug_assert!(inode.inode_type().expect("") == disk::InodeType::Symlink);
        Symlink {
            inode: inode,
            img: img,
        }
    }

    pub fn get_link(&self) -> Result<Vec<u8>> {
        get_link(self.inode, self.img.as_ref())
    }
}

fn get_link(inode: disk::Inode, img: &disk::Image) -> Result<Vec<u8>> {
    let sz = inode.size() as usize;
    if sz > LINK_TARGET_MAX {
	return Err(Error::Bounds("link target too long"));
    }
    let mut res = vec![0; sz];
    inode.read_at(res.as_mut_slice(), 0, img)?;
    Ok(res)
}

impl Iterator for ReadDir {
    type Item = Result<DirEntry>;

    fn next(&mut self) -> Option<Self::Item> {
        let res = self.dir.get(self.pos);
        self.pos += 1;
        match res {
            Err(e) => Some(Err(e)),
            Ok(None) => None,
            Ok(Some(v)) => Some(Ok(v)),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let size = (self.dir.len() - self.pos) as usize;
        (size, Some(size))
    }
}

impl FS {
    pub fn open<F: disk::ReadAt + 'static>(
        f: F,
        key: Option<&[u8]>,
        nonce: Option<&[u8]>,
    ) -> Result<FS> {
        Ok(FS {
            img: Arc::new(disk::open_file(f, key, nonce)?),
        })
    }

    pub fn open_file<P: AsRef<path::Path>>(
        path: P,
        key: Option<&[u8]>,
        nonce: Option<&[u8]>,
    ) -> Result<FS> {
        FS::open(std::fs::File::open(path)?, key, nonce)
    }

    pub fn get_root(&self) -> Result<Directory> {
        let inode = self.img.root_inode()?;
        if inode.inode_type()? != disk::InodeType::Directory {
            return Err(Error::Format("root inode is not a directory"));
        }
        Ok(Directory::new(inode, self.img.clone()))
    }

    pub fn resolve<P: AsRef<[u8]>>(&self, path: P) -> Result<Option<FSItem>> {
        resolve_dir(self.img.clone(), &self.get_root()?, path)
    }
}

fn binary_search(
    img: &disk::Image,
    inode: &disk::Inode,
    name: &[u8],
) -> Result<Option<disk::Inode>> {
    let mut min = 0;
    let mut max = inode.size() / std::mem::size_of::<disk::Dirent>() as u64;
    while min <= max {
        let mid = ((max - min) / 2) + min;
        let val = inode.read_dirent(mid, img)?;
        match name.cmp(val.name(img)?.into_bytes().as_slice()) {
            Ordering::Equal => return Ok(Some(val.inode(img)?)),
            Ordering::Less => max = mid - 1,
            Ordering::Greater => min = mid + 1,
        }
    }
    Ok(None)
}

fn resolve_path<P: AsRef<[u8]>>(
    img: &disk::Image,
    root: &disk::Inode,
    path: P,
    count: u16,
) -> Result<Option<disk::Inode>> {
    if count > LINK_LOOP_MAX {
        return Err(Error::Bounds("maximum symlink loop count encoutered"));
    }
    let path: &[u8] = path.as_ref();
    let mut cur = *root;
    if path[0] == b'/' {
        cur = img.root_inode()?;
    }
    for elem in path.split(|c| c == &b'/') {
        if cur.inode_type()? != disk::InodeType::Directory {
            return Err(Error::InvalidOperation(
                "path traversal met non-directory",
            ));
        }
        if elem.len() == 0 || elem == [b'.'] {
            continue;
        }
        if elem == [b'.', b'.'] {
            cur = cur.parent_inode(img)?;
            continue;
        }
        let new = match binary_search(img, &cur, elem)? {
            None => return Ok(None),
            Some(i) => i,
        };
        if new.inode_type()? == disk::InodeType::Symlink {
            let link_path = get_link(new, &img)?;
            cur = match resolve_path(img, &cur, link_path, count + 1)? {
                None => return Ok(None),
                Some(i) => i,
            };
            continue;
        }
        cur = new;
    }
    Ok(Some(cur))
}

fn resolve_dir<P: AsRef<[u8]>>(
    img: Arc<disk::Image>,
    root: &Directory,
    path: P,
) -> Result<Option<FSItem>> {
    let inode = resolve_path(img.as_ref(), &root.inode, path, 0)?;
    match inode {
        None => Ok(None),
        Some(i) => Ok(Some(new_fsitem(img, i)?)),
    }
}
