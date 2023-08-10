use crate::db::{memory::MemoryDB, DB};
use anyhow::anyhow;
use std::os::unix::io::FromRawFd;

pub struct UnixFileLoader<'a>(pub &'a std::path::PathBuf);

impl<'a> UnixFileLoader<'a> {
    pub fn new(filename: &'a std::path::PathBuf) -> Self {
        Self(filename)
    }

    pub async fn load(&self) -> Result<Box<MemoryDB>, anyhow::Error> {
        unsafe {
            let fd = nix::libc::open(
                std::ffi::CString::from_vec_unchecked(self.0.to_str().unwrap().as_bytes().to_vec())
                    .as_ptr(),
                nix::libc::O_RDONLY,
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

            let mut res: MemoryDB = ciborium::from_reader(std::fs::File::from_raw_fd(fd))?;
            res.update_recurrence()?;
            Ok(Box::new(res))
        }
    }

    pub async fn dump(&self, db: &mut Box<MemoryDB>) -> Result<(), anyhow::Error> {
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

            db.update_recurrence()?;

            Ok(ciborium::into_writer(
                db.as_ref(),
                std::fs::File::from_raw_fd(fd),
            )?)
        }
    }
}
