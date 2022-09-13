use crate::error::Error;
use crate::Result;

use std::os::unix::fs::FileExt;

extern crate chacha20poly1305;
use chacha20poly1305::aead::AeadInPlace;
use chacha20poly1305::aead::NewAead;
use chacha20poly1305::XChaCha20Poly1305;

pub trait Decrypter {
    fn read_at(&self, buf: &mut [u8], off: u64) -> Result<usize>;

    fn read_exact_at(&self, buf: &mut [u8], off: u64) -> Result<()> {
        if self.read_at(buf, off)? != buf.len() {
            Err(Error::IO(std::io::Error::from(
                std::io::ErrorKind::UnexpectedEof,
            )))
        } else {
            Ok(())
        }
    }
}

pub struct EncryptNone(std::fs::File);

impl EncryptNone {
    pub fn new(f: std::fs::File) -> Self {
        EncryptNone(f)
    }
}

impl Decrypter for EncryptNone {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> Result<usize> {
        Ok(self.0.read_at(buf, offset)?)
    }

    fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> Result<()> {
        Ok(self.0.read_exact_at(buf, offset)?)
    }
}

struct BlockPos(u64, u16);

const BLOCK_SIZE: u64 = 4080;
// This is true for XChaCha20-Poly1305 and AES_GCM_SIV, make sure it
// is true for any new impl too
const TAG_SIZE: u64 = 16;

const FILE_BLOCK: u64 = 4096;

pub struct EncryptXChaCha20 {
    f: std::fs::File,
    crypto: XChaCha20Poly1305,
}

impl EncryptXChaCha20 {
    pub fn new(f: std::fs::File, key: &[u8]) -> Result<Self> {
        Ok(EncryptXChaCha20 {
            f: f,
            crypto: XChaCha20Poly1305::new_from_slice(key)
                .map_err(|_| Error::Crypto("Invalid key length".into()))?,
        })
    }

    fn block_nonce(&self, block_pos: BlockPos) -> [u8; 24] {
        let mut nonce = [0; 24];
        nonce[..8].copy_from_slice(&block_pos.0.to_be_bytes());
        nonce
    }

    fn block_pos(&self, pos: u64) -> BlockPos {
        BlockPos(pos / BLOCK_SIZE, (pos % BLOCK_SIZE) as u16)
    }

    fn read_block(&self, buf: &mut Vec<u8>, block_pos: BlockPos) -> Result<()> {
        self.f
            .read_exact_at(buf.as_mut_slice(), block_pos.0 * FILE_BLOCK)?;
        let n = self.block_nonce(block_pos);
        let nonce = chacha20poly1305::XNonce::from_slice(&n);
        self.crypto
            .decrypt_in_place(nonce, b"", buf)
            .map_err(|_| Error::Crypto("XChaCha20-Poly1305: Decryption error".into()))
    }
}

impl Decrypter for EncryptXChaCha20 {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> Result<usize> {
        todo!("read_at")
    }
}
