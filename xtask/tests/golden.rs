use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("craft-launcher-xtask-golden-{label}-{}-{id}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

fn run_xtask(args: &[&str]) -> Output {
    let out = Command::new(env!("CARGO_BIN_EXE_xtask")).current_dir(root()).args(args).output().unwrap();
    assert!(
        out.status.success(),
        "args: {args:?}\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

const SAMPLE_PNGS: [&str; 3] = [
    "assets/app-icon/hicolor/16x16/apps/ai.storyteller.artcraft-launcher.png",
    "assets/app-icon/hicolor/32x32/apps/ai.storyteller.artcraft-launcher.png",
    "assets/app-icon/hicolor/64x64/apps/ai.storyteller.artcraft-launcher.png",
];

#[test]
fn version_stdout_and_dry_run_plan_match_golden() {
    let out = run_xtask(&["version"]);
    assert_eq!(out.stdout, include_bytes!("golden/version.stdout"));

    let temp = TempDir::new("version");
    let manifest_path = temp.0.join("Cargo.toml");
    let input = include_str!("golden/version.input.toml");
    let expected_updated = include_str!("golden/version.updated.toml");
    std::fs::write(&manifest_path, input).unwrap();

    let mut cfg = craft_xtask::Configs::for_root(&temp.0).version;
    cfg.refresh_lockfile = false;
    let plan = craft_xtask::version::compute(&cfg, &craft_xtask::version::Args::Set("1.2.3-rc.1".into())).unwrap();
    assert_eq!(craft_xtask::version::render(&plan), include_str!("golden/version-set.stdout"));
    assert_eq!(plan.edits.len(), 1);
    assert_eq!(plan.edits[0].2, expected_updated);
    // Dry-run compute must not write to disk before apply/run.
    assert_eq!(std::fs::read_to_string(&manifest_path).unwrap(), input);

    craft_xtask::version::run(&cfg, &["set".into(), "1.2.3-rc.1".into()]).unwrap();
    assert_eq!(std::fs::read_to_string(&manifest_path).unwrap(), expected_updated);

    let crlf = input.replace('\n', "\r\n");
    assert_eq!(craft_xtask::version::replace(&crlf, "1.2.3-rc.1").unwrap(), expected_updated.replace('\n', "\r\n"));
}

#[test]
fn ico_stdout_and_bytes_match_golden() {
    let cfg = craft_xtask::ico::Config { root: root(), ..Default::default() };
    let mut args = vec!["xtask/tests/golden/icon.ico".to_string()];
    args.extend(SAMPLE_PNGS.iter().map(|s| (*s).to_string()));
    let report = craft_xtask::ico::compute(&cfg, &craft_xtask::ico::Args::parse(&args).unwrap()).unwrap();
    assert_eq!(craft_xtask::ico::render(&report).as_bytes(), include_bytes!("golden/ico.stdout"));
    assert_eq!(report.bytes.as_slice(), include_bytes!("golden/icon.ico"));

    let temp = TempDir::new("ico");
    let out_file = temp.0.join("icon.ico");
    let out_str = out_file.to_str().unwrap();
    let cli_out = run_xtask(&["ico", out_str, SAMPLE_PNGS[0], SAMPLE_PNGS[1], SAMPLE_PNGS[2]]);
    let normalized = String::from_utf8(cli_out.stdout).unwrap().replace(out_str, "xtask/tests/golden/icon.ico");
    assert_eq!(normalized, include_str!("golden/ico.stdout"));
    assert_eq!(std::fs::read(&out_file).unwrap().as_slice(), include_bytes!("golden/icon.ico"));

    let golden_path = root().join("xtask/tests/golden/icon.ico");
    assert_eq!(craft_xtask::sha256_hex(&out_file).unwrap(), craft_xtask::sha256_hex(&golden_path).unwrap());

    let committed = std::fs::read(root().join("assets/app-icon/artcraft-launcher.ico")).unwrap();
    assert_eq!(&committed[..4], &[0, 0, 1, 0]);
    assert!(u16::from_le_bytes([committed[4], committed[5]]) >= 4);
}

#[test]
fn icns_stdout_and_bytes_match_golden() {
    let cfg = craft_xtask::ico::Config { root: root(), icns: true, ..Default::default() };
    let mut args = vec!["xtask/tests/golden/icon.icns".to_string()];
    args.extend(SAMPLE_PNGS.iter().map(|s| (*s).to_string()));
    let report = craft_xtask::ico::compute(&cfg, &craft_xtask::ico::Args::parse(&args).unwrap()).unwrap();
    assert_eq!(craft_xtask::ico::render(&report).as_bytes(), include_bytes!("golden/icns.stdout"));
    assert_eq!(report.bytes.as_slice(), include_bytes!("golden/icon.icns"));

    let temp = TempDir::new("icns");
    let out_file = temp.0.join("icon.icns");
    let out_str = out_file.to_str().unwrap();
    let cli_out = run_xtask(&["icns", out_str, SAMPLE_PNGS[0], SAMPLE_PNGS[1], SAMPLE_PNGS[2]]);
    let normalized = String::from_utf8(cli_out.stdout).unwrap().replace(out_str, "xtask/tests/golden/icon.icns");
    assert_eq!(normalized, include_str!("golden/icns.stdout"));
    assert_eq!(std::fs::read(&out_file).unwrap().as_slice(), include_bytes!("golden/icon.icns"));

    let golden_path = root().join("xtask/tests/golden/icon.icns");
    assert_eq!(craft_xtask::sha256_hex(&out_file).unwrap(), craft_xtask::sha256_hex(&golden_path).unwrap());

    let committed = std::fs::read(root().join("assets/app-icon/artcraft-launcher.icns")).unwrap();
    assert_eq!(&committed[..4], b"icns");
    assert!(committed.windows(4).any(|w| w == b"ic10"));
}

#[test]
fn help_includes_local_tasks_and_shared_syntax() {
    let out = run_xtask(&["--help"]);
    let text = String::from_utf8(out.stdout).unwrap();
    for needle in ["ci", "wasm", "local", craft_xtask::version::cli_help(), craft_xtask::ico::cli_help()] {
        assert!(text.contains(needle), "missing {needle:?} in help:\n{text}");
    }
}
