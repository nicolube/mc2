mod config;
mod convert;
mod docker;

use crate::config::Mixin;
use crate::docker::Dockerfile;
use clap::Parser;
use std::io::{BufWriter, Cursor, Write};
use std::path::PathBuf;
use std::process::Stdio;
use std::{env, io, process};
use sha2::Digest;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None, trailing_var_arg = true)]
struct Cli {
    /// Prints out generated docker file
    #[arg(short, long, default_value = "false")]
    dry_run: bool,

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

    // Search paths
    let paths = match cli.machine {
        None => Vec::from([
            PathBuf::from("mc.yaml"),
            PathBuf::from_iter([".mc", "mc.yaml"]),
        ]),
        Some(machine) => {
            let machine_file_name = format!("{}.yaml", machine);
            Vec::from([
                PathBuf::from(&machine_file_name),
                PathBuf::from_iter([".mc", &machine_file_name]),
                PathBuf::from_iter([".mc", &machine, &machine_file_name]),
            ])
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

    let dockerfile = Dockerfile::try_from(&config).expect("Failed to convert toolchain file");

    let mut buf = Cursor::new(Vec::new());
    {
        let mut buf = BufWriter::new(&mut buf);
        dockerfile.write_to(&mut buf)?;
    }
    let docker_file_data = String::from_utf8(buf.into_inner()).unwrap();

    if cli.dry_run {
        println!("{}", docker_file_data);
        return Ok(());
    } else {
        let hash = {
            let mut hasher = sha2::Sha256::new();
            Digest::update(&mut hasher, docker_file_data.as_bytes());
            hex::encode(hasher.finalize())
        };
        let tag = &format!("mini-cross2-{}", hash);

        let output = process::Command::new("docker").args([
            "images",
            "-q", tag
        ]).output()?;
        let exists = !output.stdout.is_empty();

        if exists {
            println!("Image already exists, skipping build...");
        } else {
            // Build image
            let mut build_progress = process::Command::new("docker")
                .args(["image", "build", "--tag", &tag, "-f", "-", "."])
                .stdin(Stdio::piped())
                .stdout(Stdio::inherit())
                .spawn()?;
            // Pipe dockerfile into the progress since it es read from stdin
            let stdin = build_progress.stdin.as_mut().unwrap();
            stdin.write_all(docker_file_data.as_bytes())?;
            build_progress.wait()?;
        }

        let workdir = env::current_dir()?;
        let display = env!("DISPLAY");
        process::Command::new("docker").args([
            "run",
            "--rm",
            "-it",
            "-e", &format!("DISPLAY={}", display),
            "-v", "/tmp/.X11-unix:/tmp/.X11-unix",
            "-v", &format!("{}:{}", workdir.display(), workdir.display()),
            "-w", &workdir.to_string_lossy(),
            tag,
        ]).args(cli.cmd).status()?;

    }

    Ok(())
}
