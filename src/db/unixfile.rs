use crate::db::DB;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::os::unix::io::FromRawFd;

pub struct UnixFileLoader<'a>(pub &'a std::path::PathBuf);

impl<'a> UnixFileLoader<'a> {
    pub fn new(filename: &'a std::path::PathBuf) -> Self {
        Self(filename)
    }

    pub async fn load<T>(&self) -> Result<T>
    where
        T: Serialize + for<'de> Deserialize<'de> + Default,
    {
        unsafe {
            let fd = nix::libc::open(
                std::ffi::CString::from_vec_unchecked(self.0.to_str().unwrap().as_bytes().to_vec())
                    .as_ptr(),
                nix::libc::O_RDONLY,
            );

            if fd < 0 {
                return Ok(T::default());
            }

            if nix::libc::flock(fd, nix::libc::LOCK_EX) != 0 {
                return Err(anyhow!(std::ffi::CStr::from_ptr(nix::libc::strerror(
                    nix::errno::errno()
                ))
                .to_str()
                .unwrap()
                .to_string()));
            }

            Ok(ciborium::from_reader(std::fs::File::from_raw_fd(fd))?)
        }
    }

    pub async fn dump<T>(&self, mut db: T) -> Result<()>
    where
        T: DB + Serialize + for<'de> Deserialize<'de>,
    {
        unsafe {
            let fd = nix::libc::open(
                std::ffi::CString::from_vec_unchecked(self.0.to_str().unwrap().as_bytes().to_vec())
                    .as_ptr(),
                nix::libc::O_WRONLY | nix::libc::O_TRUNC | nix::libc::O_CREAT,
            );
            if fd < 0 {
                return Err(anyhow!(std::ffi::CStr::from_ptr(nix::libc::strerror(
                    nix::errno::errno()
                ))
                .to_str()
                .unwrap()
                .to_string()));
            }

            if nix::libc::flock(fd, nix::libc::LOCK_EX) != 0 {
                return Err(anyhow!(std::ffi::CStr::from_ptr(nix::libc::strerror(
                    nix::errno::errno()
                ))
                .to_str()
                .unwrap()
                .to_string()));
            }

            if nix::libc::chmod(
                std::ffi::CString::from_vec_unchecked(self.0.to_str().unwrap().as_bytes().to_vec())
                    .as_ptr(),
                nix::libc::S_IRUSR | nix::libc::S_IWUSR,
            ) != 0
            {
                return Err(anyhow!(std::ffi::CStr::from_ptr(nix::libc::strerror(
                    nix::errno::errno()
                ))
                .to_str()
                .unwrap()
                .to_string()));
            }

            db.update_recurrence().await?;

            ciborium::into_writer(&db, std::fs::File::from_raw_fd(fd))?;
            Ok(())
        }
    }
}
