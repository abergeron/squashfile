use std::matches;

use crate::Result;

use crate::disk;
use crate::disk::u32le;
use crate::disk::u64le;
use crate::disk::CompressionType;
use crate::disk::EncryptionType;
use crate::disk::InodeType;

#[test]
fn test_u64le() {
    let val: u64 = 0x0102030405060708;
    let t: u64le = val.into();

    assert_eq!(t.val, val.to_le());

    let val2: u64 = t.into();

    assert_eq!(val, val2);
}

#[test]
fn test_u32le() {
    let val: u32 = 0x01020304;
    let t: u32le = val.into();

    assert_eq!(t.val, val.to_le());

    let val2: u32 = t.into();

    assert_eq!(val, val2);
}

#[test]
fn test_encryption_type() {
    let v: u8 = 0;
    let t = v.try_into();

    assert!(matches!(t, Ok(EncryptionType::None)));

    let v: u8 = 1;
    let t: Result<EncryptionType> = v.try_into();

    assert!(matches!(t, Ok(EncryptionType::ChaCha20)));

    let v: u8 = 2;
    let t: Result<EncryptionType> = v.try_into();

    assert!(matches!(t, Err(_)));

    let v: u8 = EncryptionType::None.into();
    assert_eq!(v, 0);

    let v: u8 = EncryptionType::ChaCha20.into();
    assert_eq!(v, 1);
}

#[test]
fn test_compression_type() {
    let v: u8 = 0;
    let t = v.try_into();

    assert!(matches!(t, Ok(CompressionType::None)));

    let v: u8 = 1;
    let t: Result<CompressionType> = v.try_into();

    assert!(matches!(t, Err(_)));

    let v: u8 = CompressionType::None.into();
    assert_eq!(v, 0);
}

#[test]
fn test_inode_type() {
    let v: u8 = 0;
    let t = v.try_into();

    assert!(matches!(t, Ok(InodeType::Directory)));

    let v: u8 = 1;
    let t = v.try_into();

    assert!(matches!(t, Ok(InodeType::File)));

    let v: u8 = 2;
    let t = v.try_into();

    assert!(matches!(t, Ok(InodeType::Symlink)));

    let v: u8 = 3;
    let t: Result<InodeType> = v.try_into();

    assert!(matches!(t, Err(_)));

    let v: u8 = InodeType::Directory.into();
    assert_eq!(v, 0);

    let v: u8 = InodeType::File.into();
    assert_eq!(v, 1);

    let v: u8 = InodeType::Symlink.into();
    assert_eq!(v, 2);
}

#[test]
fn test_open() {
    let f = std::fs::File::open("test_data/small.sqh").unwrap();
    let img = disk::open_file(f, None);
    assert!(matches!(img, Ok(_)));
}

#[test]
fn test_get_root() {
    let f = std::fs::File::open("test_data/small.sqh").unwrap();
    let img = disk::open_file(f, None).unwrap();

    let root = img.root_inode();
    assert!(matches!(root, Ok(_)));
}
