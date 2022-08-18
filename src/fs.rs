// std::fs-like interface (read-only of course)

use crate::disk;
use crate::error::Error;

use std::path;
use std::ffi::CString;
use std::iter::Iterator;

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

#[derive(Debug)]
pub struct ReadDir<'a> {
    dir: &'a Directory<'a>,
    pos: u64,
}

pub struct FS {
    img: disk::Image,
}

impl FileType {
    pub fn is_dir(&self) -> bool { self.ty == disk::InodeType::Directory }
    pub fn is_file(&self) -> bool { self.ty == disk::InodeType::File }
    pub fn is_symlink(&self) -> bool { self.ty == disk::InodeType::Symlink }
}

impl<'a> DirEntry<'a> {
    pub fn file_type(&self) -> FileType {
        FileType {
            ty: self.ent.inode(self.img).expect("").
                inode_type().expect("")
        }
    }

    pub fn file_name(&self) -> CString {
        self.ent.name(self.img).expect("")
    }

    pub fn item(&self) -> FSItem<'a> {
        let inode = self.ent.inode(self.img).expect("");
        match inode.inode_type().expect("") {
            disk::InodeType::File => FSItem::File(File::new(inode, self.img)),
            disk::InodeType::Directory => FSItem::Directory(Directory::new(inode, self.img)),
            disk::InodeType::Symlink => FSItem::Symlink(Symlink::new(inode, self.img)),
        }
    }
}

impl<'a> Directory<'a> {
    fn new(inode: disk::Inode, img: &'a disk::Image) -> Self {
        if inode.inode_type().expect("") != disk::InodeType::Directory {
            panic!("Creating a Directory with a non-directory inode");
        }
        Directory {
            inode: inode,
            img: img,
        }
    }

    pub fn len(&self) -> u64 {
        self.inode.size() / std::mem::size_of::<disk::Dirent>() as u64
    }

    pub fn get(&self, pos: u64) -> Option<DirEntry<'a>> {
        if pos >= self.len() {
            None
        } else {
            Some(DirEntry {
                ent: self.inode.read_dirent(pos, self.img).expect(""),
                img: self.img,
            })
        }
    }

    pub fn iter(&'a self) -> ReadDir<'a> {
        ReadDir {
            dir: self,
            pos: 0,
        }
    }
}

impl<'a> File<'a> {
    fn new(inode: disk::Inode, img: &'a disk::Image) -> Self {
        if inode.inode_type().expect("") != disk::InodeType::File {
            panic!("Creating a File with a non-file inode");
        }
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
        if inode.inode_type().expect("") != disk::InodeType::Symlink {
            panic!("Creating a Symlink with a non-symlink inode");
        }
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
    type Item = DirEntry<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let res = self.dir.get(self.pos);
        self.pos += 1;
        res
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let size = (self.dir.len() - self.pos) as usize;
        (size, Some(size))
    }
}

impl FS {
    pub fn open<P: AsRef<path::Path>>(path: P) -> Result<FS> {
        Ok( FS {
            img: disk::open_file(path)?,
        })
    }

    pub fn get_root<'a>(&'a self) -> Directory<'a> {
        let inode = self.img.root_inode().expect("");
        if inode.inode_type().expect("") != disk::InodeType::Directory {
            panic!("root inode is not a directory");
        }
        Directory::new(inode, &self.img)
    }
}
