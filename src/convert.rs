use crate::config::Mixin;
use crate::docker::{Command, Dockerfile, User};
use derive_more::{Display, Error};
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Error, Display, Debug)]
pub enum ConversionError {
    #[display("'base:' found in multiple files: {}, {}", a.display(), b.display())]
    MultipleBases { a: PathBuf, b: PathBuf },
    #[display("No image source has been found! Please define 'base:'")]
    NoBase,
    #[display("Invalid base: {}", _0)]
    UnknownBase(#[error(not(source))] String),
}

pub enum PackageManager {
    DNF,
    ZYPPER,
    PACMAN,
    APT,
    APK,
}

impl PackageManager {
    const fn install_prefix(&self) -> &'static str {
        match self {
            PackageManager::DNF => "dnf install -y",
            PackageManager::ZYPPER => "zypper install -y",
            PackageManager::PACMAN => "pacman -S --noconfirm",
            PackageManager::APT => "apt install -y",
            PackageManager::APK => "apk add",
        }
    }

    const fn upgrade(&self) -> &'static str {
        match self {
            PackageManager::DNF => "dnf upgrade -y",
            PackageManager::ZYPPER => "zypper update -y",
            PackageManager::PACMAN => "pacman -Syu --noconfirm",
            PackageManager::APT => "apt update && apt upgrade -y",
            PackageManager::APK => "apk update",
        }
    }

    fn defaults(&self) -> Vec<Command> {
        let mut result: Vec<Command> = Vec::from([Command::COMMENT("Ensure UTF-8 Support".into())]);
        match self {
            PackageManager::DNF => result.extend([
                self.install(&["glibc-locale-source"]),
                Command::RUN(
                    "localedef --force --inputfile=en_US --charmap=UTF-8 en_US.UTF-8".to_string(),
                ),
            ]),
            PackageManager::ZYPPER => {
                result.extend([self.install(&["glibc-locale", "glibc-i18ndata"])])
            }
            PackageManager::PACMAN => {}
            PackageManager::APT => result.extend([
                self.install(&["locales"]),
                Command::arg("DEBIAN_FRONTEND", "noninteractive"),
                Command::RUN("echo \'en_US.UTF-8 UTF-8\' >> /etc/locale.gen".to_string()),
                Command::RUN("locale-gen".to_string()),
            ]),
            PackageManager::APK => {}
        };
        result.extend([
            Command::env("LANG", "en_US.UTF-8"),
            Command::env("LANGUAGE", "en_US:en"),
            Command::env("LC_ALL", "en_US.UTF-8"),
        ]);

        result.extend([
            Command::COMMENT("Installing sudo and allow sudo for anyone".into()),
            self.install(&["sudo"]),
            Command::RUN("echo 'ALL ALL = (ALL) NOPASSWD: ALL' >> /etc/sudoers".into()),
        ]);

        result
    }

    pub fn install<T: ToString>(&self, packages: &[T]) -> Command {
        let packages = packages
            .iter()
            .map(|x| x.to_string())
            .collect::<Vec<String>>()
            .join(" ");
        Command::RUN(format!("{} {}", self.install_prefix(), packages))
    }
}

impl FromStr for PackageManager {
    type Err = ConversionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let base = s.splitn(2, ':').nth(0).unwrap();
        match base.to_lowercase().as_str() {
            "fedora" => Ok(PackageManager::DNF),
            "debian" => Ok(PackageManager::APT),
            "ubuntu" => Ok(PackageManager::APT),
            "opensuse/leap" => Ok(PackageManager::ZYPPER),
            "opensuse/tumbleweed" => Ok(PackageManager::ZYPPER),
            "archlinux" => Ok(PackageManager::PACMAN),
            "alpine" => Ok(PackageManager::APK),
            _ => Err(ConversionError::UnknownBase(s.to_string())),
        }
    }
}

/// Shell snippet that removes an existing passwd/group entry — matched by id
/// and by name — so a colliding default account can be recreated. Failures are
/// swallowed: the entry may simply not exist.
fn drop_conflicting(tool: &str, db: &str, id: u32, name: &str) -> String {
    format!(
        "{tool} \"$(getent {db} {id} | cut -d: -f1)\" 2>/dev/null || true; \
         {tool} {name} 2>/dev/null || true"
    )
}

impl TryFrom<&Mixin> for Dockerfile {
    type Error = ConversionError;

    fn try_from(value: &Mixin) -> Result<Self, Self::Error> {
        // Flatten mixins
        let mut mixins: Vec<&Mixin> = Vec::from_iter(&value.children);
        mixins.push(value);

        let mut dockerfile = Dockerfile::new();

        // Process mixins and remove duplicates
        let mut from_file: Option<&Mixin> = None;
        let mut packages: Vec<(&Mixin, Vec<String>)> = Vec::new();
        let mut scripts: Vec<(&Mixin, &String)> = Vec::new();
        for mixin in mixins {
            if mixin.yaml.base.is_some() {
                if let Some(from_file) = from_file {
                    return Err(ConversionError::MultipleBases {
                        a: from_file.path.clone(),
                        b: mixin.path.clone(),
                    });
                }
                from_file = Some(mixin)
            }
            // Filter packages so we do not try to install them twice
            let mut l_packages = Vec::new();
            for package_name in mixin.yaml.install.iter().flatten() {
                if !packages
                    .iter()
                    .map(|x| x.1.iter().any(|y| y == package_name))
                    .any(|x| x)
                {
                    l_packages.push(package_name.clone());
                }
            }
            if !l_packages.is_empty() {
                packages.push((mixin, l_packages))
            }
            if let Some(script) = &mixin.script {
                scripts.push((mixin, script));
            }

            if let Some(publish) = &mixin.yaml.publish {
                dockerfile.add_publishes(publish.iter());
            }

            if let Some(volume) = &mixin.yaml.volume {
                dockerfile.add_volumes(
                    volume
                        .iter()
                        .map(|volume| {
                            let mut volume = volume.clone();
                            volume.host_path = mixin.add_parent_path(&volume.host_path);
                            volume
                        })
                        .collect::<Vec<_>>()
                        .iter(),
                );
            }

            if let Some(env) = &mixin.yaml.env {
                for (k, v) in env {
                    dockerfile.add_env(k, v);
                }
            }
        }

        let Some(from) = &from_file else {
            return Err(ConversionError::NoBase);
        };
        let from = from.yaml.base.as_ref().unwrap().clone();
        let package_manager = PackageManager::from_str(&from)?;

        dockerfile.add(Command::FROM(from));

        dockerfile.add(Command::COMMENT(
            "Update outdated default dependencies".into(),
        ));
        dockerfile.add(Command::RUN(package_manager.upgrade().to_string()));
        dockerfile.add_all(package_manager.defaults());

        let gid = users::get_current_gid();
        let gname = users::get_current_groupname().unwrap();
        let gname = gname.display();
        let uid = users::get_current_uid();
        let uname = users::get_current_username().unwrap();
        let uname = uname.display();

        for (mixin, package_set) in &packages {
            dockerfile.add(Command::COMMENT(format!(
                "Installs from: {}",
                mixin.path.display()
            )));
            dockerfile.add(package_manager.install(package_set));
        }

        dockerfile.add(Command::COMMENT("Configure user".into()));
        // Drop any account the base image ships at our uid/gid (e.g. Ubuntu's
        // 'ubuntu' at 1000) before recreating ours. Delete the user before the
        // group, since groupdel refuses to remove a user's primary group.
        let uname = uname.to_string();
        let gname = gname.to_string();
        dockerfile.add(Command::RUN(format!(
            "{}; {}; \
             groupadd --gid {gid} {gname} && \
             useradd --gid {gid} --uid {uid} --home /home/{uname} {uname}",
            drop_conflicting("userdel -r", "passwd", uid, &uname),
            drop_conflicting("groupdel", "group", gid, &gname),
        )));
        dockerfile.add(Command::RUN(format!("mkdir -p /home/{}", uname)));
        dockerfile.add(Command::RUN(format!(
            "chown {}:{} /home/{}",
            uid, gid, uname
        )));
        dockerfile.add(Command::USER(User {
            uid: uid as u16,
            gid: Some(gid as u16),
        }));

        if let Some(parent_dir) = value.path.parent()
            && parent_dir.components().count() >= 2
        {
            let dirs = fs::read_dir(parent_dir).unwrap();
            let dirs = dirs
                .filter_map(|x| match x {
                    Ok(x) if !x.file_name().to_string_lossy().starts_with(".") => Some(x.path()),
                    _ => None,
                })
                .collect::<Vec<_>>();
            if !dirs.is_empty() {
                dockerfile.add(Command::COMMENT("Adding context dirs".into()));
            }
            for file in dirs {
                dockerfile.add(Command::COPY(
                    file.to_string_lossy().to_string(),
                    format!("/{}", file.file_name().unwrap().to_string_lossy()),
                ));
            }
        }

        for (mixin, script) in scripts {
            dockerfile.add(Command::COMMENT(format!(
                "Exec script from: {}",
                mixin.path.display()
            )));
            dockerfile.add(Command::RUN(format!("<<EOR\n/bin/sh -c {}\nEOR", script)));
        }

        dockerfile.add(Command::COMMENT("Exec bash as entrypoint".into()));
        dockerfile.add(Command::RUN("/usr/bin/env bash".into()));

        Ok(dockerfile)
    }
}
