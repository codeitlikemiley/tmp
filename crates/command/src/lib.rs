pub mod blocking {
    use std::ffi::OsStr;
    use std::io;
    use std::path::Path;
    use std::process::{Command as StdCommand, ExitStatus, Output, Stdio};

    #[derive(Debug)]
    pub struct Command {
        inner: StdCommand,
    }

    impl Command {
        pub fn new<S: AsRef<OsStr>>(program: S) -> Self {
            #[allow(unused_mut)]
            let mut inner = StdCommand::new(program);
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                inner.creation_flags(0x08000000); // CREATE_NO_WINDOW
            }
            Self { inner }
        }

        pub fn arg<S: AsRef<OsStr>>(&mut self, arg: S) -> &mut Self {
            self.inner.arg(arg);
            self
        }

        pub fn args<I, S>(&mut self, args: I) -> &mut Self
        where
            I: IntoIterator<Item = S>,
            S: AsRef<OsStr>,
        {
            self.inner.args(args);
            self
        }

        pub fn env<K, V>(&mut self, key: K, val: V) -> &mut Self
        where
            K: AsRef<OsStr>,
            V: AsRef<OsStr>,
        {
            self.inner.env(key, val);
            self
        }

        pub fn envs<I, K, V>(&mut self, vars: I) -> &mut Self
        where
            I: IntoIterator<Item = (K, V)>,
            K: AsRef<OsStr>,
            V: AsRef<OsStr>,
        {
            self.inner.envs(vars);
            self
        }

        pub fn env_remove<K: AsRef<OsStr>>(&mut self, key: K) -> &mut Self {
            self.inner.env_remove(key);
            self
        }

        pub fn env_clear(&mut self) -> &mut Self {
            self.inner.env_clear();
            self
        }

        pub fn current_dir<P: AsRef<Path>>(&mut self, dir: P) -> &mut Self {
            self.inner.current_dir(dir);
            self
        }

        pub fn stdin<T: Into<Stdio>>(&mut self, cfg: T) -> &mut Self {
            self.inner.stdin(cfg);
            self
        }

        pub fn stdout<T: Into<Stdio>>(&mut self, cfg: T) -> &mut Self {
            self.inner.stdout(cfg);
            self
        }

        pub fn stderr<T: Into<Stdio>>(&mut self, cfg: T) -> &mut Self {
            self.inner.stderr(cfg);
            self
        }

        pub fn output(&mut self) -> io::Result<Output> {
            self.inner.output()
        }

        pub fn status(&mut self) -> io::Result<ExitStatus> {
            self.inner.status()
        }

        pub fn spawn(&mut self) -> io::Result<std::process::Child> {
            self.inner.spawn()
        }
    }
}

pub use blocking::Command;
