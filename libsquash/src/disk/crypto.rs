use crate::error::Error;
use crate::Result;

use crate::disk::ReadAt;
use std::cmp::min;
use std::io::{Write, Seek};
use std::io;

extern crate chacha20;
use chacha20::cipher::IvSizeUser;
use chacha20::cipher::KeyIvInit;
use chacha20::cipher::KeySizeUser;
use chacha20::cipher::StreamCipher;
use chacha20::ChaCha20;

const CHACHA20_REKEY_PERIOD: u64 = 4_294_967_296; // 2**32
const CHACHA20_BUFFER_SIZE: usize = 4096;

pub struct EncryptChaCha20<F> {
    f: F,
    nonce_prefix: [u8; 4],
    key: chacha20::Key,
    pos: u64,
    buf: [u8; CHACHA20_BUFFER_SIZE],
}

impl<F> EncryptChaCha20<F> {
    pub fn new(f: F, key: &[u8], nonce_prefix: &[u8]) -> Result<Self> {
        if key.len() != ChaCha20::key_size() {
            return Err(Error::Crypto("Invalid key length"));
        }
        if nonce_prefix.len() != ChaCha20::iv_size() - 8 {
            return Err(Error::Crypto("Invalid nonce_prefix length"));
        }
        Ok(EncryptChaCha20 {
            f: f,
            nonce_prefix: nonce_prefix.try_into().unwrap(),
            key: *chacha20::Key::from_slice(key),
            pos: 0,
            buf: [0; CHACHA20_BUFFER_SIZE],
        })
    }

    fn block_nonce(&self, n: &mut chacha20::Nonce, pos: u64) {
        let nonce = n.as_mut_slice();
        let block_pos = pos / CHACHA20_REKEY_PERIOD;
        nonce[..4].copy_from_slice(&self.nonce_prefix);
        nonce[4..].copy_from_slice(&block_pos.to_be_bytes());
    }
}

impl<F: ReadAt> ReadAt for EncryptChaCha20<F> {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> Result<usize> {
        let mut pos = 0;
        let mut nonce = *chacha20::Nonce::from_slice(&[0; 12]);
        let sz = self.f.read_at(buf, offset)?;
        let mut len = sz;
        let mut off = offset;
        while len > 0 {
            // position inside the block
            let p = off % CHACHA20_REKEY_PERIOD;
            // length remaining (up to the size of the block)
            let l = min(len, (CHACHA20_REKEY_PERIOD - p) as usize);
            // current buffer (within the limits of the block)
            let b = &mut buf[pos..pos + l];
            self.block_nonce(&mut nonce, off);
            let mut crypto = ChaCha20::new(&self.key, &nonce);
            crypto
                .try_apply_keystream(b)
                .map_err(|_| Error::Crypto("Decrypting error"))?;
            len -= l;
            pos += l;
            off += l as u64;
        }
        Ok(sz)
    }
}

impl<W: Write> Write for EncryptChaCha20<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut pos = 0;
        let mut nonce = *chacha20::Nonce::from_slice(&[0; 12]);
        let mut len = buf.len();
        let mut off = self.pos;
        while len > 0 {
            let p = off % CHACHA20_REKEY_PERIOD;
            let l = min(len, min(CHACHA20_BUFFER_SIZE, (CHACHA20_REKEY_PERIOD - p) as usize));
            let b = &buf[pos..pos + l];
            self.block_nonce(&mut nonce, off);
            let mut crypto = ChaCha20::new(&self.key, &nonce);
            crypto.apply_keystream_b2b(b, &mut self.buf[..l]).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            let sz = self.f.write(&self.buf[..l])?;
            self.pos += sz as u64;
            if sz == 0 {
                return Ok(pos);
            }
            len -= sz;
            pos += sz;
            off += sz as u64;
        }
        Ok(pos)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.f.flush()
    }
}

impl<S: Seek> Seek for EncryptChaCha20<S> {
    fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
        self.pos = self.f.seek(pos)?;
        Ok(self.pos)
    }
}
