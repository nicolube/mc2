use crate::config::Mixin;
use crate::docker::{Command, Dockerfile};
use derive_more::{Display, Error};
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Error, Display, Debug)]
pub enum ConversionError {
    #[display("'from:' found in multiple files: {}, {}", a.display(), b.display())]
    MultibleFrom{
        a: PathBuf,
        b: PathBuf
    },
    #[display("No image source has been found! Please define 'from:'")]
    NoFrom,
    #[display("Invalid image source: {}", _0)]
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
            PackageManager::DNF => "dnf upgrade",
            PackageManager::ZYPPER => "zypper upgrade",
            PackageManager::PACMAN => "packman -Syu",
            PackageManager::APT => "apt update && apt upgrade -y",
            PackageManager::APK => "apk update"
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
                    return Err(ConversionError::MultibleFrom {
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
            return Err(ConversionError::NoFrom)
        };
        let from = from.config.base.as_ref().unwrap().clone();
        let package_manager = PackageManager::from_str(&from)?;

        let mut dockerfile = Dockerfile::new();

        dockerfile.add(Command::FROM(from));
        dockerfile.add(Command::RUN(package_manager.upgrade().to_string()));

        for (mixin, package_set) in &packages {
            let packages = package_set.join(" ");
            dockerfile.add(Command::COMMENT(format!("Installs from: {}", mixin.path.display())));
            dockerfile.add(Command::RUN(format!("{} {}", package_manager.install_prefix(), packages)));
        }

        Ok(dockerfile)
    }
}