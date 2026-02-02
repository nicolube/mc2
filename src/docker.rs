use derive_more::Display;
use std::fmt::{Display, Formatter};
use std::io::{BufWriter, Write};
use crate::convert::PackageManager;

#[derive(Debug, Clone)]
pub struct User {
    pub uid: u16,
    pub gid: Option<u16>,
}

impl Display for User {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.uid)?;
        if let Some(gid) = self.gid {
            write!(f, ":{}", gid)?;
        }
        Ok(())
    }
}

#[derive(Debug, Display, Clone)]
pub enum Command {
    #[display("FROM {}", _0)]
    FROM(String),
    #[display("# {}", _0)]
    COMMENT(String),
    #[display("CMD {}", _0)]
    CMD(String),
    #[display("ENV {}={}", _0, _1)]
    ENV(String, String),
    #[display("RUN {}", _0)]
    RUN(String),
    #[display("USER {}", _0)]
    USER(User),
    #[display("COPY {} {}", _0, _1)]
    COPY(String, String),
}

impl Command {
    pub fn env<A: ToString + ?Sized, B: ToString + ?Sized>(a: &A, b: &B) -> Self {
        Self::ENV(a.to_string(), b.to_string())
    }
}

#[derive(Debug)]
pub struct Dockerfile {
    entries: Vec<Command>,
}

impl Dockerfile {
    pub fn new() -> Self {
        Dockerfile {
            entries: Vec::new(),
        }
    }

    pub fn add(&mut self, command: Command) {
        self.entries.push(command)
    }
    pub fn add_all<I: IntoIterator<Item = Command>>(&mut self, commands: I) {
        self.entries.extend(commands)
    }

    pub fn write_to<T: Write>(&self, writer: &mut BufWriter<T>) -> std::io::Result<()> {
        for entry in self.entries.iter() {
            if matches!(entry, Command::COMMENT(_)) {
                write!(writer, "\n")?;
            }
            write!(writer, "{}\n", entry)?;
        }

        Ok(())
    }
}
