mod mixin;

use crate::docker::Dockerfile;
use derive_more::{Display, Error, From};
pub use mixin::*;
use serde::{Deserialize, Serialize};
use serde_with::{DeserializeFromStr, SerializeDisplay};
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io::BufReader;
use std::num::ParseIntError;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::{env, io};

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct UserConfig {
    publish: Vec<Publish>,
    volume: Vec<Volume>,
    env: HashMap<String, String>,
}

impl UserConfig {
    pub fn load() -> io::Result<Self> {
        let home = env::home_dir();
        let current = env::current_dir()?;

        let configs: Vec<UserConfig> = home
            .map(|path| {
                [
                    path.join(PathBuf::from(".mc2config.yaml")),
                    path.join(PathBuf::from_iter([".config", "mc2", "config.yaml"])),
                ]
            })
            .into_iter()
            .chain([[
                current.join(".mc2config.yaml"),
                current.join(PathBuf::from_iter([".mc2", ".mc2config.yaml"])),
            ]])
            .flatten()
            .filter_map(|path| {
                if path.exists() && path.is_file() {
                    let mut config: UserConfig =
                        serde_yaml::from_reader(BufReader::new(File::open(&path).ok()?)).ok()?;
                    match &path.parent() {
                        None => {}
                        Some(parent) => config.volume.iter_mut().for_each(|volume| {
                            if volume.host_path.is_relative() {
                                volume.host_path = parent.join(path.clone())
                            }
                        }),
                    }
                    Some(Ok(config))
                } else {
                    None
                }
            })
            .collect::<io::Result<Vec<_>>>()?;
        let mut result = UserConfig::default();
        for config in configs {
            result.publish.extend(config.publish);
            result.volume.extend(config.volume);
            result.env.extend(config.env);
        }
        Ok(result)
    }
    pub fn append_docker(&self, dockerfile: &mut Dockerfile) {
        dockerfile.add_publishes(self.publish.iter());
        dockerfile.add_volumes(self.volume.iter());
        for (k, v) in &self.env {
            dockerfile.add_env(k, v);
        }
    }
}

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
    #[display(
        "Invalid publish format: <host_path>:<machine_path>[:<ro|readonly|volume-nocopy,..>]"
    )]
    InvalidFormat,
}

#[derive(Debug, Clone, PartialEq, Eq, DeserializeFromStr, SerializeDisplay)]
pub struct Volume {
    pub host_path: PathBuf,
    pub machine_path: PathBuf,
    pub opts: Vec<String>,
}

impl Display for Volume {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}",
            self.host_path.display(),
            self.machine_path.display()
        )?;
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
                machine_path,
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
            if !path.exists() || !path.is_file() {
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

    #[test]
    fn test_user_config() {
        let expected = UserConfig {
            env: HashMap::from([("A".into(), "B".into())]),
            publish: Vec::from(["8080:80".parse().unwrap()]),
            volume: Vec::from(["/usr/bin/test:/bin".parse().unwrap()]),
        };

        let yaml = serde_yaml::to_string(&expected).unwrap();
        println!("{}", yaml);
    }
}
