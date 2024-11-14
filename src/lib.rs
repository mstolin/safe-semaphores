use bitflags::bitflags;
use libc::{sem_t, O_CREAT, O_EXCL, S_IRWXG, S_IRWXO, S_IRWXU};
use std::{ffi::CString, io::Error};

// TO SIMPLIFY THING, ONLY
bitflags! {
    /// Represents the permission mode for a semaphore file.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct SemFSMode: u32 {
        /// All three permissions for the group category. Equally to `070`.
        const GROUP = S_IRWXG;
        /// All three permissions for the other category. Equally to `07`.
        const OTHER = S_IRWXO;
        /// All three permissions for the user category. Equally to `0700`.
        const USER = S_IRWXU;
        /// The combination for all permission for the categories group, other, and user.
        /// Equally to `777`.
        const ALL = Self::GROUP.bits() | Self::OTHER.bits() | Self::USER.bits();
    }
}

bitflags! {
    /// A bitmask that represents if a semaphore will be opened or created.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct SemOFlags: i32 {
        /// Create a new semaphore.
        const CREATE = O_CREAT;
        /// Open an existing semaphore.
        const OPEN = 0;
        /// Create a new semaphore exclusively. If it already exists, `sem_open` fails.
        const CREATE_EXCL = Self::CREATE.bits() | O_EXCL;
    }
}

// #[derive(Send, Sync)]
pub struct NamedSemaphore {
    raw: *mut sem_t,
}

impl NamedSemaphore {
    pub unsafe fn create(
        name: &str,
        mode: SemFSMode,
        value: u32,
        create_exclusive: bool,
    ) -> std::io::Result<Self> {
        let name = CString::new(name.as_bytes())?;

        let oflags = if create_exclusive {
            SemOFlags::CREATE_EXCL
        } else {
            SemOFlags::CREATE
        };

        let raw = libc::sem_open(name.as_ptr(), oflags.bits(), mode.bits(), value);
        if raw == libc::SEM_FAILED {
            return Err(std::io::Error::last_os_error());
        }

        Ok(Self { raw })
    }

    pub unsafe fn open(name: &str) -> std::io::Result<Self> {
        let name = CString::new(name.as_bytes())?;
        let raw = libc::sem_open(name.as_ptr(), SemOFlags::OPEN.bits());
        if raw == libc::SEM_FAILED {
            return Err(std::io::Error::last_os_error());
        }
        Ok(Self { raw })
    }

    pub unsafe fn open_or_create(
        name: &str,
        mode: SemFSMode,
        value: u32,
        create_exclusive: bool,
    ) -> std::io::Result<Self> {
        match Self::open(name) {
            Ok(raw) => Ok(raw),
            _ => Self::create(name, mode, value, create_exclusive),
        }
    }

    pub unsafe fn post(&self) -> std::io::Result<()> {
        let res = libc::sem_post(self.raw);
        if res == -1 {
            return Err(Error::last_os_error());
        }
        Ok(())
    }

    pub unsafe fn wait(&self) -> std::io::Result<()> {
        let res = libc::sem_wait(self.raw);
        if res == -1 {
            return Err(Error::last_os_error());
        }
        Ok(())
    }

    pub unsafe fn get_value(&self) -> std::io::Result<i32> {
        let mut val: i32 = 0;
        let res = libc::sem_getvalue(self.raw, &mut val);
        if res == -1 {
            return Err(Error::last_os_error());
        }
        Ok(val)
    }
}

impl Drop for NamedSemaphore {
    fn drop(&mut self) {
        // Result is ignored
        let _ = unsafe { libc::sem_close(self.raw) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs::{self, File},
        io::Write,
        path::Path,
    };

    const SHM_PATH: &str = "/dev/shm";
    const SEM_NAME: &str = "TEST_SEM";

    fn get_sem_path() -> String {
        format!("{}/sem.{}", SHM_PATH.to_string(), SEM_NAME.to_string())
    }

    fn does_sem_exist() -> bool {
        Path::new(&get_sem_path()).exists()
    }

    fn remove_sem() -> Result<(), std::io::Error> {
        fs::remove_file(Path::new(&get_sem_path()))
    }

    #[test]
    fn create_sem() {
        assert_eq!(does_sem_exist(), false, "semaphore shouldn't already exist");
        let sem = unsafe { NamedSemaphore::create(SEM_NAME, SemFSMode::all(), 0, true) };
        assert!(sem.is_ok());
        assert_eq!(does_sem_exist(), true, "semaphore was not created");
        std::mem::drop(sem);
        let _ = remove_sem();
        assert_eq!(
            does_sem_exist(),
            false,
            "semaphore should have been removed"
        );
    }

    #[test]
    fn create_post_wait() {
        assert_eq!(does_sem_exist(), false, "semaphore shouldn't already exist");
        let sem = unsafe {
            NamedSemaphore::create(SEM_NAME, SemFSMode::all(), 0, true)
                .expect("couldn't create semaphore")
        };
        assert_eq!(does_sem_exist(), true, "semaphore was not created");

        let val = unsafe { sem.get_value().unwrap() };
        assert_eq!(val, 0);
        let _ = unsafe { sem.post() };
        let val = unsafe { sem.get_value().unwrap() };
        assert_eq!(val, 1);
        let _ = unsafe { sem.wait() };
        let val = unsafe { sem.get_value().unwrap() };
        assert_eq!(val, 0);

        let _ = remove_sem();
        assert_eq!(
            does_sem_exist(),
            false,
            "semaphore should have been removed"
        );
    }
}
