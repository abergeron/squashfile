// std::fs-like interface (read-only of course)

use crate::disk;
use crate::error::Error;

use std::cmp::Ordering;
use std::ffi::CString;
use std::iter::Iterator;
use std::path;

type Result<T> = std::result::Result<T, Error>;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct FileType {
    ty: disk::InodeType,
}

#[derive(Copy, Clone)]
pub struct DirEntry<'a> {
    img: &'a disk::Image,
    ent: disk::Dirent,
}

#[derive(Copy, Clone, Debug)]
pub struct Directory<'a> {
    img: &'a disk::Image,
    inode: disk::Inode,
}

#[derive(Copy, Clone, Debug)]
pub struct File<'a> {
    img: &'a disk::Image,
    inode: disk::Inode,
}

#[derive(Copy, Clone, Debug)]
pub struct Symlink<'a> {
    img: &'a disk::Image,
    inode: disk::Inode,
}

pub enum FSItem<'a> {
    File(File<'a>),
    Directory(Directory<'a>),
    Symlink(Symlink<'a>),
}

fn new_fsitem<'a>(img: &'a disk::Image, inode: disk::Inode) -> Result<FSItem<'a>> {
    Ok(match inode.inode_type()? {
        disk::InodeType::File => FSItem::File(File::new(inode, img)),
        disk::InodeType::Directory => FSItem::Directory(Directory::new(inode, img)),
        disk::InodeType::Symlink => FSItem::Symlink(Symlink::new(inode, img)),
    })
}

#[derive(Debug)]
pub struct ReadDir<'a> {
    dir: &'a Directory<'a>,
    pos: u64,
}

pub struct FS {
    img: disk::Image,
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

impl<'a> DirEntry<'a> {
    pub fn file_type(&self) -> Result<FileType> {
        Ok(FileType {
            ty: self.ent.inode(self.img)?.inode_type()?,
        })
    }

    pub fn file_name(&self) -> Result<CString> {
        self.ent.name(self.img)
    }

    pub fn item(&self) -> Result<FSItem<'a>> {
        let inode = self.ent.inode(self.img)?;
        Ok(new_fsitem(self.img, inode)?)
    }
}

impl<'a> Directory<'a> {
    fn new(inode: disk::Inode, img: &'a disk::Image) -> Self {
        std::debug_assert!(inode.inode_type().expect("") == disk::InodeType::Directory);
        Directory {
            inode: inode,
            img: img,
        }
    }

    pub fn len(&self) -> u64 {
        self.inode.size() / std::mem::size_of::<disk::Dirent>() as u64
    }

    pub fn resolve<P: AsRef<[u8]>>(&self, path: P) -> Result<Option<FSItem<'a>>> {
        resolve_dir(&self.img, self, path)
    }

    pub fn get(&self, pos: u64) -> Result<Option<DirEntry<'a>>> {
        if pos >= self.len() {
            Ok(None)
        } else {
            Ok(Some(DirEntry {
                ent: self.inode.read_dirent(pos, self.img)?,
                img: self.img,
            }))
        }
    }

    pub fn iter(&'a self) -> ReadDir<'a> {
        ReadDir { dir: self, pos: 0 }
    }
}

impl<'a> File<'a> {
    fn new(inode: disk::Inode, img: &'a disk::Image) -> Self {
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
        self.inode.read_at(buf, offset, self.img)
    }
}

impl<'a> Symlink<'a> {
    fn new(inode: disk::Inode, img: &'a disk::Image) -> Self {
        std::debug_assert!(inode.inode_type().expect("") == disk::InodeType::Symlink);
        Symlink {
            inode: inode,
            img: img,
        }
    }

    pub fn get_link(&self) -> Result<Vec<u8>> {
        let mut res = Vec::with_capacity(self.inode.size() as usize);
        self.inode.read_at(res.as_mut_slice(), 0, self.img)?;
        Ok(res)
    }
}

impl<'a> Iterator for ReadDir<'a> {
    type Item = Result<DirEntry<'a>>;

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
    pub fn open<P: AsRef<path::Path>>(path: P) -> Result<FS> {
        Ok(FS {
            img: disk::open_file(path)?,
        })
    }

    pub fn get_root<'a>(&'a self) -> Result<Directory<'a>> {
        let inode = self.img.root_inode()?;
        if inode.inode_type()? != disk::InodeType::Directory {
            return Err(Error::Format("root inode is not a directory".into()));
        }
        Ok(Directory::new(inode, &self.img))
    }

    pub fn resolve<'a, P: AsRef<[u8]>>(&'a self, path: P) -> Result<Option<FSItem<'a>>> {
        resolve_dir(&self.img, &self.get_root()?, path)
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
) -> Result<Option<disk::Inode>> {
    let path: &[u8] = path.as_ref();
    let mut cur = *root;
    if path[0] == b'/' {
        cur = img.root_inode()?;
    }
    for elem in path.split(|c| c == &b'/') {
        if cur.inode_type()? != disk::InodeType::Directory {
            return Err(Error::InvalidOperation(
                "path traversal met non-directory".into(),
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
            let mut link_path = Vec::with_capacity(new.size() as usize);
            new.read_at(link_path.as_mut_slice(), 0, img)?;
            cur = match resolve_path(img, &cur, link_path)? {
                None => return Ok(None),
                Some(i) => i,
            };
            continue;
        }
        cur = new;
    }
    Ok(Some(cur))
}

fn resolve_dir<'a, P: AsRef<[u8]>>(
    img: &'a disk::Image,
    root: &Directory<'a>,
    path: P,
) -> Result<Option<FSItem<'a>>> {
    let inode = resolve_path(img, &root.inode, path)?;
    match inode {
        None => Ok(None),
        Some(i) => Ok(Some(new_fsitem(img, i)?)),
    }
}
