use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    time::{SystemTime, UNIX_EPOCH},
};

const VERSION: &str = "9.8.7-test";

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "foundation-install-sh-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn checked_output(program: &str, args: &[&str], current_dir: &std::path::Path) -> Output {
    let output = Command::new(program)
        .args(args)
        .current_dir(current_dir)
        .output()
        .unwrap_or_else(|error| panic!("could not run {program}: {error}"));
    assert!(
        output.status.success(),
        "{program} {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

#[test]
fn installs_archive_and_enables_hermes_plugin() {
    let temp = TestDir::new();
    let root = &temp.0;
    let release = root.join("release");
    let payload = release.join("foundation");
    let install_dir = root.join("hermes-home");
    let fake_bin = root.join("fake-bin");
    let hermes_log = root.join("hermes-args");

    fs::create_dir_all(payload.join("bin")).unwrap();
    fs::create_dir_all(payload.join("adapters/hermes/plugin")).unwrap();
    fs::create_dir_all(payload.join("adapters/shared")).unwrap();
    fs::write(payload.join("bin/foundation"), "new foundation binary\n").unwrap();
    fs::write(payload.join("VERSION"), format!("{VERSION}\n")).unwrap();
    fs::write(
        payload.join("adapters/hermes/plugin/manifest.yaml"),
        "name: foundation\n",
    )
    .unwrap();
    fs::write(
        payload.join("adapters/shared/content.md"),
        "new shared tree\n",
    )
    .unwrap();
    checked_output(
        "chmod",
        &["+x", payload.join("bin/foundation").to_str().unwrap()],
        root,
    );

    let archive = "foundation-x86_64-unknown-linux-musl.tar.gz";
    checked_output("tar", &["-czf", archive, "foundation"], &release);
    let checksum = checked_output("sha256sum", &[archive], &release);
    let checksum_text = String::from_utf8(checksum.stdout).unwrap();
    fs::write(
        release.join(format!("{archive}.sha256")),
        format!(
            "{}  {archive}\n",
            checksum_text.split_whitespace().next().unwrap()
        ),
    )
    .unwrap();

    fs::create_dir_all(&fake_bin).unwrap();
    let hermes = fake_bin.join("hermes");
    fs::write(
        &hermes,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$HERMES_TEST_LOG\"\nprintf 'fake hermes enabled\\n'\n",
    )
    .unwrap();
    checked_output("chmod", &["+x", hermes.to_str().unwrap()], root);

    fs::create_dir_all(install_dir.join("bin")).unwrap();
    fs::create_dir_all(install_dir.join("foundation/obsolete")).unwrap();
    fs::write(install_dir.join("bin/foundation"), "old binary\n").unwrap();
    fs::write(
        install_dir.join("foundation/obsolete/old.txt"),
        "old shared tree\n",
    )
    .unwrap();

    let base_url = format!("file://{}", release.display());
    let path = format!(
        "{}:{}",
        fake_bin.display(),
        std::env::var_os("PATH")
            .unwrap_or_default()
            .to_string_lossy()
    );
    let output = Command::new("sh")
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/install.sh"))
        .args(["--hermes-home", install_dir.to_str().unwrap()])
        .env("FOUNDATION_BASE_URL", &base_url)
        .env("PATH", path)
        .env("HERMES_TEST_LOG", &hermes_log)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "install.sh exited {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stdout.contains(&format!("Downloading {archive} (latest)")));
    assert!(stdout.contains("SHA256 verified."));
    assert!(stdout.contains(&format!("Installed foundation {VERSION}")));
    assert!(stdout.contains(&format!(
        "binary: {}",
        install_dir.join("bin/foundation").display()
    )));
    assert!(stdout.contains(&format!(
        "files : {}",
        install_dir.join("foundation").display()
    )));
    assert!(stderr.is_empty(), "unexpected stderr: {stderr}");

    assert_eq!(
        fs::read(install_dir.join("bin/foundation")).unwrap(),
        b"new foundation binary\n"
    );
    assert_eq!(
        fs::read_to_string(install_dir.join("foundation/VERSION")).unwrap(),
        format!("{VERSION}\n")
    );
    assert_eq!(
        fs::read_to_string(install_dir.join("foundation/adapters/shared/content.md")).unwrap(),
        "new shared tree\n"
    );
    assert!(!install_dir.join("foundation/obsolete").exists());
    assert_eq!(
        fs::read_to_string(&hermes_log).unwrap(),
        "plugins\nenable\nfoundation\n"
    );
}
