mod mixin;

use derive_more::{Display, Error, From};
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io::BufReader;
use std::num::ParseIntError;
use std::path::PathBuf;
use std::str::FromStr;
use serde_with::{DeserializeFromStr, SerializeDisplay};
pub use mixin::*;

#[derive(Debug, Display, Error, From)]
pub enum ParsePublishError {
    IntParseError(#[error(source)] ParseIntError),
    #[display("Invalid publish format [<host_ip>:]<host_port>:<machine_prot>")]
    InvalidFormat,
}

#[derive(Debug, Clone, PartialEq, Eq, DeserializeFromStr, SerializeDisplay)]
pub struct Publish {
    pub host_ip: Option<String>,
    pub host_port: u16,
    pub machine_port: u16,
}

impl Display for Publish {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(host_ip) = &self.host_ip {
            write!(f, "{}:", host_ip)?;
        }
        write!(f, "{}:{}", self.host_port, self.machine_port)
    }
}

impl FromStr for Publish {
    type Err = ParsePublishError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let split = s.split(':').collect::<Vec<_>>();
        if split.len() >= 2 {
            let host_port = u16::from_str(split[split.len() - 2])?;
            let machine_port = u16::from_str(split[split.len() - 1])?;
            if split.len() == 2 {
                return Ok(Publish {
                    host_port,
                    machine_port,
                    host_ip: None,
                });
            } else if split.len() == 3 {
                return Ok(Publish {
                    host_port,
                    machine_port,
                    host_ip: Some(split[0].to_string()),
                });
            }
        }
        Err(ParsePublishError::InvalidFormat)
    }
}

#[derive(Debug, Display, Error, From)]
pub enum ParseVolumeError {
    #[display("Invalid publish format: <host_path>:<machine_path>[:<ro|readonly|volume-nocopy,..>]")]
    InvalidFormat,

}

#[derive(Debug, Clone, PartialEq, Eq, DeserializeFromStr, SerializeDisplay)]
pub struct Volume {
    pub host_path: PathBuf,
    pub machine_path: PathBuf ,
    pub opts: Vec<String>,
}

impl Display for Volume {

    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.host_path.display(), self.machine_path.display())?;
        if !self.opts.is_empty() {
            write!(f, ":{}", self.opts.join(","))?;
        }
        Ok(())
    }

}

impl FromStr for Volume {
    type Err = ParseVolumeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let splits = s.split(":").collect::<Vec<_>>();
        if splits.len() == 2 || splits.len() == 3 {
            let host_path = PathBuf::from(splits[0]);
            let machine_path = PathBuf::from(splits[1]);
            let mut opts = Vec::new();
            if splits.len() == 3 {
                opts = splits[2].split(",").map(String::from).collect::<Vec<_>>();
                for opt in &opts {
                    if !["ro", "readonly", "volume-nocopy"].contains(&opt.as_str()) {
                        return Err(ParseVolumeError::InvalidFormat);
                    }
                }
            }
            return Ok(Self {
                opts,
                host_path,
                machine_path
            });
        }
        Err(ParseVolumeError::InvalidFormat)
    }
}

pub fn get_alias_from_config(machine: &str) -> Option<PathBuf> {
    {
        [
            PathBuf::from(".mc2aliases.yaml"),
            PathBuf::from_iter([".mc", ".mc2aliases.yaml"]),
        ]
        .into_iter()
        .find_map(|path| {
            if !path.exists() {
                return None;
            }
            let read = BufReader::new(File::open(&path).unwrap());
            let aliases: HashMap<String, PathBuf> = serde_yaml::from_reader(read).unwrap();
            aliases.get(machine).map(|target| {
                let mut target = match path.parent() {
                    Some(path) => path.join(target),
                    None => target.clone(),
                };
                target.set_extension("yaml");
                target
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_publish_3() {
        let raw = "127.0.0.1:8080:80";
        let expected = Publish {
            host_ip: Some("127.0.0.1".to_string()),
            host_port: 8080,
            machine_port: 80,
        };
        assert_eq!(Publish::from_str(raw).unwrap(), expected);
        assert_eq!(&expected.to_string(), raw);
    }

    #[test]
    fn test_parse_publish_2() {
        let raw = "8080:80";
        let expected = Publish {
            host_ip: None,
            host_port: 8080,
            machine_port: 80,
        };
        assert_eq!(Publish::from_str(raw).unwrap(), expected);
        assert_eq!(&expected.to_string(), raw);
    }

    #[test]
    fn test_parse_volume() {
        let raw = "/opt/custom_data:my_app/data";
        let expected = Volume {
            host_path: "/opt/custom_data".into(),
            machine_path: "my_app/data".into(),
            opts: Vec::new(),
        };
        assert_eq!(Volume::from_str(raw).unwrap(), expected);
        assert_eq!(&expected.to_string(), raw);
    }

    #[test]
    fn test_parse_volume_opts() {
        let raw = "/opt/custom_data:my_app/data:ro,volume-nocopy";
        let expected = Volume {
            host_path: "/opt/custom_data".into(),
            machine_path: "my_app/data".into(),
            opts: Vec::from_iter(["ro", "volume-nocopy"].into_iter().map(String::from)),
        };
        assert_eq!(Volume::from_str(raw).unwrap(), expected);
        assert_eq!(&expected.to_string(), raw);
    }
}
