use std::fmt::{Display, Formatter};
use std::io::{BufWriter, Write};
use derive_more::Display;


#[derive(Debug)]
pub struct User {
    pub uid: u16,
    pub gid: Option<u16>
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

#[derive(Debug, Display)]
pub enum Command {
    #[display("FROM {}", _0)]
    FROM(String),
    #[display("# {}", _0)]
    COMMENT(String),
    #[display("RUN {}", _0)]
    RUN(String),
    #[display("USER {}", _0)]
    USER(User),
    #[display("COPY {} {}", _0, _1)]
    COPY(String, String),

}

#[derive(Debug)]
pub struct Dockerfile {
    entries: Vec<Command>
}

impl Dockerfile {

    pub fn new() -> Self {
        Dockerfile{
            entries: Vec::new()
        }
    }

    pub fn add(&mut self, command: Command) {
        self.entries.push(command)
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