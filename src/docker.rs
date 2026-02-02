use std::io::{BufWriter, Write};
use derive_more::Display;

#[derive(Debug, Display)]
pub enum Command {
    #[display("from: {}", _0)]
    FROM(String),
    #[display("# {}", _0)]
    COMMENT(String),
    #[display("RUN {}", _0)]
    RUN(String),
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
            write!(writer, "{}\n", entry)?;
            if !matches!(entry, Command::COMMENT(_)) {
                write!(writer, "\n")?;
            }
        }

        Ok(())
    }
}