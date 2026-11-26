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
    let external_dirs_file = root.join("external-dirs.json");

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
    fs::write(&external_dirs_file, "[\"/existing/skills\"]\n").unwrap();
    let hermes = fake_bin.join("hermes");
    fs::write(
        &hermes,
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$HERMES_TEST_LOG\"\ncase \"$1:$2\" in\n  config:get)\n    case \"${HERMES_TEST_MODE:-}\" in\n      get-fail) printf 'config get failed for skills.external_dirs\\n' >&2; exit 3 ;;\n      invalid-json) printf '{invalid\\n'; exit 0 ;;\n      invalid-shape) printf '%s\\n' '{\"private-setting\":\"do not expose\"}'; exit 0 ;;\n      missing-config) printf '[]\\n'; exit 0 ;;\n    esac\n    if [ -f \"$HERMES_TEST_EXTERNAL_DIRS_FILE\" ]; then cat \"$HERMES_TEST_EXTERNAL_DIRS_FILE\"; else printf '[]\\n'; fi ;;\n  config:set)\n    if [ \"${HERMES_TEST_MODE:-}\" = set-fail ]; then printf 'config set rejected skills.external_dirs\\n' >&2; exit 4; fi\n    printf '%s\\n' \"$4\" > \"$HERMES_TEST_EXTERNAL_DIRS_FILE\" ;;\n  plugins:enable) printf 'plugins:\\n  enabled:\\n    - foundation\\nother-setting: preserved\\n' > \"$HERMES_HOME/config.yaml\" ;;\n  *) exit 2 ;;\nesac\n",
    )
    .unwrap();
    checked_output("chmod", &["+x", hermes.to_str().unwrap()], root);

    fs::create_dir_all(install_dir.join("bin")).unwrap();
    fs::create_dir_all(install_dir.join("foundation/obsolete")).unwrap();
    fs::write(
        install_dir.join("config.yaml"),
        "plugins:\n  enabled: []\nother-setting: preserved\n",
    )
    .unwrap();
    fs::write(
        install_dir.join("shell-hooks-allowlist.json"),
        "{\"allowlist\": []}\n",
    )
    .unwrap();
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
        .env("PATH", &path)
        .env("HERMES_TEST_LOG", &hermes_log)
        .env("HERMES_TEST_EXTERNAL_DIRS_FILE", &external_dirs_file)
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
        format!(
            "config get skills.external_dirs --json\nconfig set skills.external_dirs [\"/existing/skills\", \"{}\"]\nplugins enable foundation\n",
            install_dir.join("foundation/skills").display()
        )
    );
    assert_eq!(
        fs::read_to_string(&external_dirs_file).unwrap(),
        format!(
            "[\"/existing/skills\", \"{}\"]\n",
            install_dir.join("foundation/skills").display()
        )
    );
    assert_eq!(
        fs::read_to_string(install_dir.join("config.yaml")).unwrap(),
        "plugins:\n  enabled:\n    - foundation\nother-setting: preserved\n"
    );
    assert_eq!(
        fs::read_to_string(install_dir.join("shell-hooks-allowlist.json")).unwrap(),
        "{\"allowlist\": []}\n"
    );
    let rerun = Command::new("sh")
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/install.sh"))
        .args(["--hermes-home", install_dir.to_str().unwrap()])
        .env("FOUNDATION_BASE_URL", &base_url)
        .env("PATH", &path)
        .env("HERMES_TEST_LOG", &hermes_log)
        .env("HERMES_TEST_EXTERNAL_DIRS_FILE", &external_dirs_file)
        .output()
        .unwrap();
    assert!(
        rerun.status.success(),
        "idempotent reinstall failed: {}",
        String::from_utf8_lossy(&rerun.stderr)
    );
    let baseline_external_dirs = fs::read(&external_dirs_file).unwrap();
    for (mode, expected) in [
        ("get-fail", "config get failed for skills.external_dirs"),
        ("invalid-json", "invalid JSON at line 1, column 2"),
        (
            "invalid-shape",
            "expected skills.external_dirs to be a JSON list of strings",
        ),
        ("set-fail", "config set rejected skills.external_dirs"),
    ] {
        let failed = Command::new("sh")
            .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/install.sh"))
            .args(["--hermes-home", install_dir.to_str().unwrap()])
            .env("FOUNDATION_BASE_URL", &base_url)
            .env("PATH", &path)
            .env("HERMES_TEST_EXTERNAL_DIRS_FILE", &external_dirs_file)
            .env("HERMES_TEST_LOG", &hermes_log)
            .env("HERMES_TEST_MODE", mode)
            .output()
            .unwrap();
        assert!(!failed.status.success(), "{mode} should fail");
        let stderr = String::from_utf8_lossy(&failed.stderr);
        assert!(stderr.contains(expected), "{mode}: {stderr}");
        assert!(
            !stderr.contains("private-setting") && !stderr.contains("do not expose"),
            "{mode} leaked unrelated config: {stderr}"
        );
        assert_eq!(
            fs::read(&external_dirs_file).unwrap(),
            baseline_external_dirs
        );
    }

    let missing_config_home = root.join("missing-config-home");
    assert!(!missing_config_home.join("config.yaml").exists());
    let missing_config_external_dirs = root.join("missing-config-external-dirs.json");
    let missing_config = Command::new("sh")
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/install.sh"))
        .args(["--hermes-home", missing_config_home.to_str().unwrap()])
        .env("FOUNDATION_BASE_URL", &base_url)
        .env("PATH", &path)
        .env(
            "HERMES_TEST_EXTERNAL_DIRS_FILE",
            &missing_config_external_dirs,
        )
        .env("HERMES_TEST_LOG", &hermes_log)
        .env("HERMES_TEST_MODE", "missing-config")
        .output()
        .unwrap();
    assert!(
        missing_config.status.success(),
        "missing config was not initialized: {}",
        String::from_utf8_lossy(&missing_config.stderr)
    );
    let missing_config_dirs: serde_json::Value =
        serde_json::from_slice(&fs::read(&missing_config_external_dirs).unwrap()).unwrap();
    assert_eq!(
        missing_config_dirs,
        serde_json::json!([missing_config_home
            .join("foundation/skills")
            .display()
            .to_string()])
    );
    assert!(missing_config_home.join("config.yaml").exists());

    assert!(stdout.contains("hermes plugins disable foundation"));

    // Re-run with all installer utilities available except for Hermes itself.
    let no_hermes_bin = root.join("no-hermes-bin");
    fs::create_dir_all(&no_hermes_bin).unwrap();
    for utility in [
        "curl",
        "tar",
        "gzip",
        "sha256sum",
        "install",
        "mktemp",
        "python3",
        "uname",
        "cat",
        "mkdir",
        "mv",
        "rm",
        "ln",
        "sh",
    ] {
        let Some(path) = std::env::var_os("PATH").and_then(|paths| {
            std::env::split_paths(&paths)
                .map(|dir| dir.join(utility))
                .find(|path| path.is_file())
        }) else {
            panic!("missing test utility {utility}");
        };
        #[cfg(unix)]
        std::os::unix::fs::symlink(path, no_hermes_bin.join(utility)).unwrap();
    }
    let missing_home = root.join("missing-hermes-home");
    let missing = Command::new("sh")
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/install.sh"))
        .args(["--hermes-home", missing_home.to_str().unwrap()])
        .env("FOUNDATION_BASE_URL", &base_url)
        .env("PATH", &no_hermes_bin)
        .output()
        .unwrap();
    assert!(!missing.status.success());
    let missing_stderr = String::from_utf8_lossy(&missing.stderr);
    assert!(
        missing_stderr.contains("hermes CLI not found"),
        "unexpected missing-Hermes stderr: {missing_stderr}"
    );
    assert!(missing_home.join("bin/foundation").is_file());
    assert!(missing_home.join("foundation/VERSION").is_file());

    let physical_home = root.join("physical-plugin-home");
    let physical_plugin = physical_home.join("plugins/foundation");
    fs::create_dir_all(&physical_plugin).unwrap();
    fs::write(
        physical_plugin.join("owned.txt"),
        "leave this plugin alone\n",
    )
    .unwrap();
    let physical_before = fs::read(physical_plugin.join("owned.txt")).unwrap();
    let physical_log = root.join("physical-hermes-args");
    let physical = Command::new("sh")
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/install.sh"))
        .args(["--hermes-home", physical_home.to_str().unwrap()])
        .env("FOUNDATION_BASE_URL", &base_url)
        .env("PATH", &path)
        .env("HERMES_TEST_LOG", &physical_log)
        .env("HERMES_TEST_EXTERNAL_DIRS_FILE", &external_dirs_file)
        .output()
        .unwrap();
    assert!(!physical.status.success());
    let physical_stderr = String::from_utf8_lossy(&physical.stderr);
    assert!(
        physical_stderr.contains("not a symlink")
            || physical_stderr.contains("not a symbolic link"),
        "unexpected physical-plugin stderr: {physical_stderr}"
    );
    assert_eq!(
        fs::read(physical_plugin.join("owned.txt")).unwrap(),
        physical_before
    );
    assert!(!physical_log.exists(), "Hermes CLI must not be called");
    assert!(physical_home.join("foundation/VERSION").is_file());
}

#[test]
fn rejects_removed_enable_options() {
    for option in ["--enable-hooks", "--enable-plugin"] {
        let output = Command::new("sh")
            .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/install.sh"))
            .arg(option)
            .output()
            .unwrap();
        assert!(!output.status.success(), "{option} unexpectedly succeeded");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(&format!("unknown argument: {option}")));
    }
}
