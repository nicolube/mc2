use derive_more::Display;
use sha2::Digest;
use std::fmt::{Display, Formatter};
use std::io::{BufWriter, Cursor, ErrorKind, Write};
use std::process::Stdio;
use std::{env, io, process};

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
    #[display("ARG {}={}", _0, _1)]
    ARG(String, String),
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
    
    pub fn arg<A: ToString + ?Sized, B: ToString + ?Sized>(a: &A, b: &B) -> Self {
        Self::ARG(a.to_string(), b.to_string())
    }
}

#[derive(Debug)]
pub struct Dockerfile {
    entries: Vec<Command>,
    publish: Vec<String>,
    volumes: Vec<String>,
}

impl Dockerfile {
    pub fn new() -> Self {
        Dockerfile {
            entries: Vec::new(),
            publish: Vec::new(),
            volumes: Vec::new(),
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

    pub fn hash(&self) -> String {
        let mut hasher = sha2::Sha256::new();
        Digest::update(&mut hasher, self.to_string().as_bytes());
        hex::encode(hasher.finalize())
    }

    pub fn tag(&self) -> String {
        format!("mini-cross2-{}", self.hash())
    }

    pub fn exists(&self) -> io::Result<bool> {
        let tag = self.tag();
        let output = process::Command::new("docker")
            .args(["images", "-q", &tag])
            .output()?;
        Ok(!output.stdout.is_empty() && output.status.success())
    }

    pub fn build(&self) -> io::Result<()> {
        let tag = self.tag();
        // Build image
        let mut build_progress = process::Command::new("docker")
            .args(["image", "build", "--tag", &tag, "-f", "-", "."])
            .stdin(Stdio::piped())
            .stdout(Stdio::inherit())
            .spawn()?;
        // Pipe dockerfile into the progress since it es read from stdin
        let stdin = build_progress.stdin.as_mut().unwrap();
        self.write_to(&mut BufWriter::new(stdin))?;
        if !build_progress.wait()?.success() {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "Failed to build docker image",
            ));
        }
        Ok(())
    }

    pub fn run(
        &self,
        cmd: &Vec<String>,
        publish: &Vec<String>,
        volumes: &Vec<String>,
    ) -> io::Result<()> {
        let tag = self.tag();

        let workdir = env::current_dir()?;
        let display = env!("DISPLAY");
        let publish = publish.iter().chain(self
            .publish.iter())
            .into_iter()
            .map(|x| ["-p", x.as_str()])
            .flatten()
            .collect::<Vec<&str>>();
        let volumes = volumes
            .iter()
            .chain(self.volumes.iter())
            .into_iter()
            .map(|x| ["-v", x.as_str()])
            .flatten()
            .collect::<Vec<&str>>();
        process::Command::new("docker")
            .args([
                "run",
                "--rm",
                "-it",
                "-e",
                &format!("DISPLAY={}", display),
                "-v",
                "/tmp/.X11-unix:/tmp/.X11-unix",
                "-v",
                &format!("{}:{}", workdir.display(), workdir.display()),
                "-w",
                &workdir.to_string_lossy(),
            ])
            .args(publish)
            .args(volumes)
            .arg(&tag)
            .args(cmd)
            .status()?;
        Ok(())
    }
}

impl Display for Dockerfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut buf = Cursor::new(Vec::new());
        {
            let mut buf = BufWriter::new(&mut buf);
            self.write_to(&mut buf).unwrap();
        }
        write!(f, "{}", String::from_utf8(buf.into_inner()).unwrap())
    }
}
