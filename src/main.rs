mod config;
mod convert;
mod docker;

use crate::config::{Mixin, Publish, UserConfig, Volume};
use crate::docker::Dockerfile;
use clap::Parser;
use std::io;
use std::io::{BufWriter, stdout};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None, trailing_var_arg = true)]
struct Cli {
    /// Prints out generated docker file
    #[arg(short, long, default_value = "false")]
    dry_run: bool,

    /// Forces rebuild of docker image
    #[arg(short = 'F', long, default_value = "false")]
    force: bool,

    /// Force selects toolchain file
    #[arg(short, long)]
    file: Option<PathBuf>,

    /// Mound volumes, will be forwarded to docker run.
    #[arg(short, long)]
    volumes: Vec<Volume>,

    /// Published ports, will be forwarded to docker run.
    #[arg(short, long)]
    publish: Vec<Publish>,

    /// Environment variable, will be forwarded to docker run.
    #[arg(short, long)]
    env: Vec<String>,

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

impl Cli {
    pub fn append_docker(&self, dockerfile: &mut Dockerfile) -> bool {
        dockerfile.add_publishes(self.publish.iter());
        dockerfile.add_volumes(self.volumes.iter());
        for env in &self.env {
            let split = env.split_once('=');
            if let Some((k, v)) = split {
                dockerfile.add_env(k, v);
            } else {
                eprintln!("Invalid environment variable: {:?}", env);
                return false;
            }
        }
        true
    }
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();

    let path = match &cli.file {
        Some(path) => path.clone(),
        None => {
            // Load alias file path from alias file if it exists
            let alias_file: Option<PathBuf> = match &cli.machine {
                Some(machine) => config::get_alias_from_config(machine),
                None => None,
            };

            // Search paths
            let paths = match &cli.machine {
                None => Mixin::lookup_path_unnamed(),
                Some(machine) => Vec::from_iter(
                    alias_file
                        .into_iter()
                        .chain(Mixin::lookup_paths_named(machine)),
                ),
            };

            // Find the first config that exists
            let Some(path) = paths.iter().find_map(|path| {
                if path.exists() {
                    Some(path.clone())
                } else {
                    None
                }
            }) else {
                eprintln!("toolchain not found in:");
                for path in paths.iter() {
                    eprintln!("- {}", &path.display());
                }
                return Ok(());
            };
            path
        }
    };

    // Load config
    let config = match Mixin::load(&path) {
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
    UserConfig::load()?.append_docker(&mut dockerfile);
    if !cli.append_docker(&mut dockerfile) {
        return Ok(());
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
