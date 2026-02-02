use std::fs::Metadata;
use std::io;
use crate::config::Mixin;
use crate::docker::{Command, Dockerfile, User};
use derive_more::{Display, Error};
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Error, Display, Debug)]
pub enum ConversionError {
    #[display("'base:' found in multiple files: {}, {}", a.display(), b.display())]
    MultipleBases {
        a: PathBuf,
        b: PathBuf
    },
    #[display("No image source has been found! Please define 'base:'")]
    NoBase,
    #[display("Invalid base: {}", _0)]
    UnknownBase(#[error(not(source))] String),
    #[display("Dummy")]
    Dummy

}

enum PackageManager {
    DNF,
    ZYPPER,
    PACMAN,
    APT,
    APK
}

impl PackageManager {

    const fn install_prefix(&self) -> &'static str {
        match self {
            PackageManager::DNF => "dnf install -y",
            PackageManager::ZYPPER => "zypper install -y",
            PackageManager::PACMAN => "packman -S --noconfirm",
            PackageManager::APT => "apt install -y",
            PackageManager::APK => "apk add",
        }
    }

    const fn upgrade(&self) -> &'static str {
        match self {
            PackageManager::DNF => "dnf upgrade -y",
            PackageManager::ZYPPER => "zypper upgrade -y",
            PackageManager::PACMAN => "packman -Syu",
            PackageManager::APT => "apt update && apt upgrade -y",
            PackageManager::APK => "apk update --noconfirm"
        }
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
            "opensuse" => Ok(PackageManager::ZYPPER),
            "suse" => Ok(PackageManager::ZYPPER),
            "arch" => Ok(PackageManager::PACMAN),
            "alpine" => Ok(PackageManager::APK),
            _ => Err(ConversionError::UnknownBase(s.to_string()))
        }
    }
}

impl TryFrom<&Mixin> for Dockerfile {
    type Error = ConversionError;

    fn try_from(value: &Mixin) -> Result<Self, Self::Error> {
        let mut mixins: Vec<&Mixin> = Vec::from_iter(&value.children);
        mixins.push(value);
        let mut from_file: Option<&Mixin> = None;
        let mut packages: Vec<(&Mixin, Vec<String>)> = Vec::new();
        for mixin in &value.children {
            if mixin.config.base.is_some() {
                if let Some(from_file) = from_file {
                    return Err(ConversionError::MultipleBases {
                        a: from_file.path.clone(),
                        b: mixin.path.clone()
                    })
                }
                from_file = Some(mixin)
            }
            // Filter packages so we do not try to install them twice
            let mut l_packages = Vec::new();
            for package_name in mixin.config.install.iter().flatten() {
                if !packages.iter().map(|x| x.1.iter().any(|y| y == package_name)).any(|x| x) {
                    l_packages.push(package_name.clone());
                }
            }
            if !l_packages.is_empty() {
                packages.push((mixin, l_packages))
            }
        }

        let Some(from) = &from_file else {
            return Err(ConversionError::NoBase)
        };
        let from = from.config.base.as_ref().unwrap().clone();
        let package_manager = PackageManager::from_str(&from)?;

        let mut dockerfile = Dockerfile::new();

        dockerfile.add(Command::FROM(from));

        dockerfile.add(Command::COMMENT("Update outdated default dependencies".into()));
        dockerfile.add(Command::RUN(package_manager.upgrade().to_string()));

        for (mixin, package_set) in &packages {
            let packages = package_set.join(" ");
            dockerfile.add(Command::COMMENT(format!("Installs from: {}", mixin.path.display())));
            dockerfile.add(Command::RUN(format!("{} {}", package_manager.install_prefix(), packages)));
        }

        // TODO Add exec scripts

        let gid = users::get_current_gid();
        let gname = users::get_current_groupname().unwrap();
        let gname = gname.display();
        let uid = users::get_current_uid();
        let uname = users::get_current_username().unwrap();
        let uname = uname.display();

        dockerfile.add(Command::COMMENT("Configure user".into()));
        dockerfile.add(Command::RUN(format!("groupadd --gid {} {}", gid, gname)));
        dockerfile.add(Command::RUN(format!("useradd --gid {} --uid {} --home /home/{} {}", gid, uid, uname, uname)));
        dockerfile.add(Command::RUN(format!("mkdir -p /home/{}", uname)));
        dockerfile.add(Command::RUN(format!("chown {}:{} /home/{}", uid, gid, uname)));
        dockerfile.add(Command::USER(User{
            uid: uid as u16,
            gid: Some(gid as u16)
        }));

        dockerfile.add(Command::COMMENT("Exec bash as entrypoint".into()));
        dockerfile.add(Command::RUN("/usr/bin/env bash".into()));

        Ok(dockerfile)
    }
}