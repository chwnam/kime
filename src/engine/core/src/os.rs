use crate::InputCategory;
use std::io;

pub trait OsContext {
    fn read_global_hangul_state(&mut self) -> io::Result<InputCategory>;
    fn update_layout_state(&mut self, category: InputCategory) -> io::Result<()>;
}

#[cfg(unix)]
mod unix {
    use crate::InputCategory;
    use std::{
        io::{self, Read, Write},
        os::unix::net::UnixStream,
        path::PathBuf,
        time::Duration,
    };

    pub struct OsContext {
        sock_path: PathBuf,
    }

    fn get_state_dir() -> PathBuf {
        let run_path = kime_run_dir::get_run_dir();
        run_path.join("kime-indicator.sock")
    }

    impl Default for OsContext {
        fn default() -> Self {
            Self {
                sock_path: get_state_dir(),
            }
        }
    }

    impl super::OsContext for OsContext {
        fn read_global_hangul_state(&mut self) -> io::Result<InputCategory> {
            let mut buf = [0; 1];
            let mut client = UnixStream::connect(&self.sock_path)?;
            client.set_read_timeout(Some(Duration::from_secs(2))).ok();
            client.set_write_timeout(Some(Duration::from_secs(2))).ok();
            client.read_exact(&mut buf)?;
            match buf[0] {
                b'1' => Ok(InputCategory::Hangul),
                _ => Ok(InputCategory::Latin),
            }
        }

        fn update_layout_state(&mut self, category: InputCategory) -> io::Result<()> {
            let category = match category {
                InputCategory::Hangul => 1,
                InputCategory::Latin => 0,
            };

            let mut client = UnixStream::connect(&self.sock_path)?;
            client.set_read_timeout(Some(Duration::from_secs(2))).ok();
            client.set_write_timeout(Some(Duration::from_secs(2))).ok();
            client.write_all(&[category])
        }
    }
}

mod fallback {
    use crate::InputCategory;
    use std::io;

    #[derive(Default)]
    pub struct OsContext;

    impl super::OsContext for OsContext {
        fn read_global_hangul_state(&mut self) -> io::Result<InputCategory> {
            Err(io::Error::new(io::ErrorKind::Other, "Unsupported platform"))
        }

        fn update_layout_state(&mut self, _category: InputCategory) -> io::Result<()> {
            Err(io::Error::new(io::ErrorKind::Other, "Unsupported platform"))
        }
    }
}

#[cfg(unix)]
use unix as imp;

#[cfg(not(unix))]
use fallback as imp;

pub use imp::OsContext as DefaultOsContext;
