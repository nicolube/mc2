mod config;
mod convert;
mod docker;

use crate::config::Mixin;
use crate::docker::Dockerfile;
use clap::Parser;
use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::io::{BufReader, BufWriter, Cursor, stdout};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None, trailing_var_arg = true)]
struct Cli {
    /// Prints out generated docker file
    #[arg(short, long, default_value = "false")]
    dry_run: bool,

    /// Forces rebuild of docker image
    #[arg(short = 'F', default_value = "false")]
    force: bool,

    /// Mound volumes, will be forwarded to docker run.
    #[arg(short, long)]
    volumes: Vec<String>,

    /// Published ports, will be forwarded to docker run.
    #[arg(short, long)]
    publish: Vec<String>,

    /// Name of environment,
    /// Config will be searched at:
    /// mc.yml,
    /// .mc/mc.yaml,
    /// <machine>.yaml,
    /// .mc/<machine>.yaml,
    /// .mc/<machine>/<machine>.yaml
    machine: Option<String>,

    /// Command that is executed after the container is up
    cmd: Vec<String>,
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();

    let alias_file: Option<PathBuf> = {
        [
            PathBuf::from(".mc2aliases.yaml"),
            PathBuf::from_iter([".mc", ".mc2aliases.yaml"]),
        ]
        .into_iter()
        .find_map(|path| {
            if cli.machine.is_none() || !path.exists() {
                return None;
            }
            let read = BufReader::new(File::open(&path).unwrap());
            let aliases: HashMap<String, PathBuf> = serde_yaml::from_reader(read).unwrap();
            aliases
                .get(&cli.machine.clone().unwrap())
                .map(|target|  {
                    let mut target = match path.parent() {
                        Some(path) => path.join(target),
                        None => target.clone(),
                    };
                    target.set_extension("yaml");
                    target
                })
        })
    };

    // Search paths
    let paths = match cli.machine {
        None => Vec::from([
            PathBuf::from("mc.yaml"),
            PathBuf::from_iter([".mc", "mc.yaml"]),
        ]),
        Some(machine) => {
            let machine_file_name = format!("{}.yaml", machine);
            Vec::from_iter(alias_file.into_iter().chain([
                PathBuf::from(&machine_file_name),
                PathBuf::from_iter([".mc", &machine_file_name]),
                PathBuf::from_iter([".mc", &machine, &machine_file_name]),
            ]))
        }
    };

    // Find the first config that exists
    let Some(path) = paths
        .iter()
        .find_map(|path| if path.exists() { Some(path) } else { None })
    else {
        eprintln!("toolchain not found in:");
        for path in paths.iter() {
            eprintln!("- {}", &path.display());
        }
        return Ok(());
    };

    // Load config
    let config = match Mixin::load(path) {
        Ok(config) => config,
        Err(e) => {
            eprintln!(
                "Failed to load toolchain file ({}):\r\n{}",
                path.display(),
                e
            );
            return Ok(());
        }
    };

    let mut dockerfile = Dockerfile::try_from(&config).expect("Failed to convert toolchain file");
    dockerfile.add_publishes(cli.publish.iter());
    dockerfile.add_volumes(cli.volumes.iter());

    let mut buf = Cursor::new(Vec::new());
    {
        let mut buf = BufWriter::new(&mut buf);
        dockerfile.write_to(&mut buf)?;
    }

    if cli.dry_run {
        dockerfile.write_to(&mut BufWriter::new(stdout()))?;
        return Ok(());
    } else {
        if dockerfile.exists()? && !cli.force {
            println!("Image already exists, skipping build...");
        } else {
            if cli.force {
                println!("Force rebuild of image...");
            }
            dockerfile.build()?;
        }
        dockerfile.run(&cli.cmd)?;
    }

    Ok(())
}
