use std::env;

mod tree;

pub fn run(args: &[String]) -> i32 {
    match tree::run(args) {
        Ok(output) => {
            if !output.is_empty() {
                println!("{output}");
            }
            0
        }
        Err((code, message)) => {
            eprintln!("{message}");
            code
        }
    }
}

pub(crate) fn home() -> Result<std::path::PathBuf, (i32, String)> {
    env::var_os("HERMES_HOME")
        .map(std::path::PathBuf::from)
        .filter(|p| p.is_absolute())
        .ok_or((1, "HERMES_HOME must be an absolute path".into()))
}
