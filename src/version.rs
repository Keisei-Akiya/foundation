use std::{env, fs, path::PathBuf};

fn hermes_home(args: &[String]) -> Result<PathBuf, String> {
    let mut explicit = None;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--hermes-home" => {
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --hermes-home".to_string())?;
                explicit = Some(PathBuf::from(value));
            }
            other => return Err(format!("unknown version argument: {other}")),
        }
    }

    if let Some(path) = explicit {
        return Ok(path);
    }
    if let Some(path) = env::var_os("HERMES_HOME") {
        return Ok(PathBuf::from(path));
    }
    env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".hermes"))
        .ok_or_else(|| "HOME is not set; use --hermes-home to specify the Hermes home".to_string())
}

pub fn run(args: &[String]) -> i32 {
    let home = match hermes_home(args) {
        Ok(home) => home,
        Err(error) => {
            eprintln!("{error}");
            eprintln!("usage: foundation version [--hermes-home DIR]");
            return 2;
        }
    };

    let binary_version = env!("CARGO_PKG_VERSION");
    let version_path = home.join("foundation").join("VERSION");
    let shared_version = match fs::read_to_string(&version_path) {
        Ok(version) => version.trim().to_string(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            println!("Binary version: {binary_version}");
            println!("Shared tree version: unavailable (VERSION file not found)");
            return 0;
        }
        Err(error) => {
            eprintln!("could not read {}: {error}", version_path.display());
            return 1;
        }
    };

    println!("Binary version: {binary_version}");
    println!("Shared tree version: {shared_version}");
    if binary_version != shared_version {
        eprintln!(
            "Warning: binary version {binary_version} does not match shared tree version {shared_version}"
        );
    }
    0
}
