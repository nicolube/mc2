use serde::Deserialize;
use std::fs::File;
use std::io;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct Mixin {
    pub path: PathBuf,
    pub config: Config,
    pub children: Vec<Mixin>,
    pub script: Option<String>,
}

impl Mixin {
    pub fn load<P: AsRef<Path>>(path: P) -> io::Result<Mixin> {
        let path: &Path = path.as_ref();
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut mixin = Mixin::try_from((path, reader))?;

        let mut children = Vec::new();
        load_mixins(&mixin, &mut children)?;
        mixin.children = children;

        Ok(mixin)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub base: Option<String>,
    pub install: Option<Vec<String>>,
    pub mixin: Option<Vec<PathBuf>>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            base: None,
            install: None,
            mixin: None,
        }
    }
}

impl<T> TryFrom<(&Path, BufReader<T>)> for Mixin
where
    T: Read,
{
    type Error = io::Error;

    /// Parses file like this
    /// ---
    /// some config
    /// ---
    /// some script
    fn try_from(value: (&Path, BufReader<T>)) -> Result<Mixin, io::Error> {
        let (path, mut reader) = value;
        // Read the entire input into a string
        let mut content = String::new();
        reader.read_to_string(&mut content)?;

        // Fast path: if no leading marker, the whole file is script
        let content = content.replace("\r\n", "\n");
        let mut it = content.lines();
        match it.next() {
            Some(first) if first.trim() == "---" => {
                // Find the closing '---' strictly; config must end with dashes
                let mut cfg_lines: Vec<&str> = Vec::new();
                let mut found_end = false;
                for l in &mut it {
                    if l.trim() == "---" {
                        found_end = true;
                        break;
                    }
                    cfg_lines.push(l);
                }

                // If closing marker not found, return a format error: config must end with dashes
                if !found_end {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "config section started with --- but missing closing ---",
                    ));
                }

                let config: Config = serde_yaml::from_str(&cfg_lines.join("\n")).map_err(|e| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("invalid config yaml: {e}"),
                    )
                })?;

                // Remaining lines are script
                let script_rest = it.collect::<Vec<_>>().join("\n");
                let script = if script_rest.is_empty() {
                    None
                } else {
                    Some(script_rest)
                };

                Ok(Mixin {
                    path: path.to_path_buf(),
                    config,
                    script,
                    children: Vec::new(),
                })
            }
            Some(first) => {
                // No config header; all is script
                let script = std::iter::once(first)
                    .chain(it)
                    .collect::<Vec<_>>()
                    .join("\n");
                let script = if script.is_empty() {
                    None
                } else {
                    Some(script)
                };

                Ok(Mixin {
                    path: path.to_path_buf(),
                    config: Config::default(),
                    script,
                    children: Vec::new(),
                })
            }
            None => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "config was empty",
            )),
        }
    }
}

fn normalized_path(mixin: &Mixin, path: &Path) -> PathBuf {
    let parent_path = mixin.path.parent();
    let file_name = &format!("{}.yaml", path.file_name().unwrap().display());
    let path = match path.parent() {
        Some(parent) => parent.join(file_name),
        None => PathBuf::from(file_name),
    };
    PathBuf::from_iter(match parent_path {
        None => path.clone(),
        Some(parent) => parent.join(path),
    }.components())
}

fn load_mixins(parent: &Mixin, children: &mut Vec<Mixin>) -> io::Result<()> {
    let Some(paths) = &parent.config.mixin else {
        return Ok(());
    };

    for path in paths {
        let path = normalized_path(parent, &path);
        let file = File::open(&path)?;
        let reader = BufReader::new(file);
        let mixin = Mixin::try_from((path.as_path(), reader))?;
        if children.iter().any(|x| &x.path == &path) {
            continue;
        }
        load_mixins(&mixin, children)?;
        children.push(mixin);
    }

    Ok(())


}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::path::Path;

    fn to_reader(s: &str) -> BufReader<Cursor<Vec<u8>>> {
        BufReader::new(Cursor::new(s.as_bytes().to_vec()))
    }

    #[test]
    fn valid_config_and_script() {
        let input = "---\nfrom: ubuntu:22.04\ninstall:\n  - curl\n  - git\nmixin: []\nworkdir: /app\n---\necho hello\n";
        let reader = to_reader(input);
        let path = Path::new("/tmp/test.mc");
        let mixin = Mixin::try_from((path, reader)).expect("should parse");
        assert_eq!(mixin.path, path.to_path_buf());
        assert_eq!(mixin.config.base.as_deref(), Some("ubuntu:22.04"));
        assert_eq!(
            mixin.config.install,
            Some(vec!["curl".into(), "git".into()])
        );
        assert_eq!(
            mixin.script.as_deref(),
            Some("echo hello\n".trim_end_matches('\n'))
        );
    }

    #[test]
    fn valid_config_no_script() {
        let input = "---\nfrom: alpine:3.20\n---\n";
        let reader = to_reader(input);
        let path = Path::new("/tmp/test2.mc");
        let mixin = Mixin::try_from((path, reader)).expect("should parse");
        assert_eq!(mixin.config.base.as_deref(), Some("alpine:3.20"));
        assert!(mixin.script.is_none());
    }

    #[test]
    fn no_config_all_script() {
        let input = "echo one\necho two\n";
        let reader = to_reader(input);
        let path = Path::new("/tmp/script.only");
        let mixin = Mixin::try_from((path, reader)).expect("should parse");
        assert_eq!(mixin.config.base, None);
        assert_eq!(mixin.config.install, None);
        assert_eq!(
            mixin.script.as_deref(),
            Some("echo one\necho two\n".trim_end_matches('\n'))
        );
    }

    #[test]
    fn missing_closing_dashes_errors() {
        let input = "---\nfrom: ubuntu\n# missing closing dashes\necho hi\n";
        let reader = to_reader(input);
        let path = Path::new("/tmp/bad.mc");
        let err = Mixin::try_from((path, reader)).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn empty_file_errors() {
        let input = "";
        let reader = to_reader(input);
        let path = Path::new("/tmp/empty.mc");
        let err = Mixin::try_from((path, reader)).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn crlf_normalization() {
        let input = "---\r\nfrom: alpine\r\n---\r\necho hi\r\n";
        let reader = to_reader(input);
        let path = Path::new("/tmp/crlf.mc");
        let mixin = Mixin::try_from((path, reader)).expect("should parse");
        assert_eq!(mixin.config.base.as_deref(), Some("alpine"));
        assert_eq!(mixin.script.as_deref(), Some("echo hi"));
    }
}
