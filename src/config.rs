mod mixin;

use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
pub use mixin::*;

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
                aliases
                    .get(machine)
                    .map(|target|  {
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