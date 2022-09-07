
extern crate libsquash;
use libsquash::fs;
use libsquash::error::Error;

use pyo3::create_exception;
use pyo3::prelude::*;
use pyo3::exceptions::{PyIsADirectoryError, PyFileNotFoundError, PyNotADirectoryError};
use pyo3::types::PyBytes;
use pyo3::types::PyUnicode;
use std::path::PathBuf;

create_exception!(pysquash, SquashError, pyo3::exceptions::PyException);

#[pyclass(module="pysquash.pysquash")]
struct SquashCursor {
    dir: fs::Directory,
}

#[pyclass(module="pysquash.pysquash")]
struct SquashFile {
    f: fs::File,
    pos: u64,
}

#[pyclass(module="pysquash.pysquash")]
struct SquashDirIter {
    rd: fs::ReadDir,
}


fn convert_err(e: Error) -> PyErr {
    match e {
        Error::IO(e) => e.into(),
        Error::Format(m) => SquashError::new_err(format!("Invalid value: {m}")),
        Error::Bounds(m) => SquashError::new_err(format!("Value out of bounds: {m}")),
        Error::InvalidOperation(m) => SquashError::new_err(format!("Invalid operation: {m}")),
    }
}

#[pymethods]
impl SquashCursor {
    #[new]
    fn new(path: &PyUnicode) -> PyResult<Self> {
        let p: PathBuf = path.extract()?;
        Ok(SquashCursor {
            dir: fs::FS::open(p).map_err(convert_err)?.get_root().map_err(convert_err)?,
        })
    }

    // file-like
    fn open(&self, path: &PyBytes) -> PyResult<SquashFile> {
        let p: &[u8] = path.extract()?;
        match self.dir.resolve(p).map_err(convert_err)? {
            Some(fs::FSItem::File(f)) => Ok(SquashFile { f: f, pos: 0 }),
            Some(_) => Err(PyIsADirectoryError::new_err(p.to_owned())),
            None => Err(PyFileNotFoundError::new_err(p.to_owned())),
        }
    }

    // sub-fs
    fn cd(&self, path: &PyBytes) -> PyResult<SquashCursor> {
        let p: &[u8] = path.extract()?;
        match self.dir.resolve(p).map_err(convert_err)? {
            Some(fs::FSItem::Directory(d)) => Ok(SquashCursor { dir: d }),
            Some(_) => Err(PyNotADirectoryError::new_err(p.to_owned())),
            None => Err(PyFileNotFoundError::new_err(p.to_owned())),
        }
    }

    // Iterator
    fn scandir(&self) -> SquashDirIter {
        SquashDirIter { rd: self.dir.iter() }
    }

    fn __iter__(&self) ->  SquashDirIter {
        self.scandir()
    }
}

impl SquashFile {
    fn read<'py>(&mut self, py: Python<'py>, size: usize) -> PyResult<&'py PyBytes> {
        let res = PyBytes::new_with(py, size,
                          |buf| self.f.read_at(buf, self.pos).map_err(convert_err));
        self.pos += size as u64;
        res
    }

    fn readall<'py>(&mut self, py: Python<'py>) -> PyResult<&'py PyBytes> {
        self.read(py, self.size() as usize)
    }

    fn readinto(&mut self, b: &mut [u8]) -> PyResult<Option<usize>> {
        self.f.read_at(b, self.pos).map_err(convert_err)?;
        self.pos += b.len() as u64;
        Ok(Some(b.len()))
    }

    fn size(&self) -> u64 {
        self.f.size()
    }
}

impl SquashDirIter {
    fn __iter__(slf: PyRef<Self>) -> PyRef<Self> {
        slf
    }

    fn __next__(&mut self, py: Python<'_>) -> PyResult<Option<PyBytes>> {
       match self.rd.next() {
            None => Ok(None),
            Some(Err(e)) => Err(convert_err(e)),
            Some(Ok(v)) => Ok(Some(PyBytes::new(py, v.file_name().map_err(convert_err)?.as_bytes()).into()))
        }
    }
}

/// A Python module implemented in Rust.
#[pymodule]
fn pysquash(py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<SquashCursor>()?;
    m.add_class::<SquashFile>()?;
    m.add_class::<SquashDirIter>()?;
    m.add("SquashError", py.get_type::<SquashError>())?;
    Ok(())
}
