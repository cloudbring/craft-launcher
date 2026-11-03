//! Workspace tooling: `cargo xtask <command>`.
//!
//! Shared `craft-xtask` tasks (`version`, `ico`, `icns`) plus launcher-specific local tasks.
//! External tools (`cargo`) are invoked through `std::process::Command`.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod local;

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const USAGE: &str = "\
usage: cargo xtask <command>

commands:
  ci              fmt --check, clippy -D warnings, test, wasm (stops at first failure)
  wasm            cargo check --target wasm32-unknown-unknown for the wasm-safe crates
  version [set X.Y.Z[-pre]]
                  print the workspace version, or set it (Cargo.toml + Cargo.lock)
  ico <out.ico> <in.png>...
                  pack square PNGs (<= 256 px) into a Windows .ico (see packaging/icons.sh)
  icns <out.icns> <in.png>...
                  pack square PNGs (16-1024 px) into a macOS .icns, on any OS
  local sync | serve | status
                  build the Craft apps from their checkouts and install them from this machine
                  (cargo xtask local help)
";

/// Packages that must build for wasm32-unknown-unknown: the pure core and the web app (which
/// pulls in the shared egui UI).
const WASM_PACKAGES: &[&str] = &["artcraft-launcher-core", "artcraft-launcher-web"];

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rest: Vec<&str> = args.iter().skip(1).map(String::as_str).collect();
    let task_args = args.get(1..).unwrap_or_default();
    let shared = craft_xtask::Configs::for_root(root());
    let ico_cfg = craft_xtask::ico::Config::default();
    let result = match args.first().map(String::as_str) {
        Some("ci") => cmd_ci(),
        Some("wasm") => cmd_wasm(),
        Some("version") => craft_xtask::version::run(&shared.version, task_args),
        Some("ico") => craft_xtask::ico::run(&ico_cfg, task_args),
        Some("icns") => craft_xtask::ico::run_icns(&ico_cfg, task_args),
        Some("local") => local::run(&rest),
        Some("-h" | "--help" | "help") | None => {
            print!("{}", usage());
            Ok(())
        }
        Some(other) => Err(format!("unknown command `{other}`\n\n{}", usage())),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn usage() -> String {
    format!("{USAGE}\nShared task syntax:\n  {}\n  {}\n", craft_xtask::version::cli_help(), craft_xtask::ico::cli_help())
}

/// Workspace root (parent of the xtask crate).
pub fn root() -> PathBuf {
    let dir = std::env::var_os("CARGO_MANIFEST_DIR").map(PathBuf::from).filter(|dir| dir.join("Cargo.toml").is_file());
    let dir = dir.unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    dir.parent().unwrap_or(Path::new(".")).to_path_buf()
}

pub fn cargo() -> Command {
    let mut c = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
    c.current_dir(root());
    c
}

fn run(mut cmd: Command, what: &str) -> Result<(), String> {
    eprintln!("$ {what}");
    let status = cmd.status().map_err(|e| format!("{what}: failed to spawn: {e}"))?;
    if status.success() { Ok(()) } else { Err(format!("{what}: exited with {status}")) }
}

fn cargo_step(args: &[&str]) -> Result<(), String> {
    let mut c = cargo();
    c.args(args);
    run(c, &format!("cargo {}", args.join(" ")))
}

fn cmd_wasm() -> Result<(), String> {
    let mut results = Vec::new();
    for pkg in WASM_PACKAGES {
        let ok = cargo_step(&["check", "--target", "wasm32-unknown-unknown", "-p", pkg]).is_ok();
        results.push((*pkg, ok));
    }
    println!("\nwasm32-unknown-unknown check:");
    for (p, ok) in &results {
        println!("  {:<6} {p}", if *ok { "ok" } else { "FAIL" });
    }
    let failed = results.iter().filter(|r| !r.1).count();
    if failed == 0 { Ok(()) } else { Err(format!("{failed} crate(s) failed the wasm check")) }
}

fn cmd_ci() -> Result<(), String> {
    type Step = (&'static str, Box<dyn Fn() -> Result<(), String>>);
    let steps: Vec<Step> = vec![
        ("fmt", Box::new(|| cargo_step(&["fmt", "--all", "--", "--check"]))),
        ("clippy", Box::new(|| cargo_step(&["clippy", "--workspace", "--all-targets", "--", "-D", "warnings"]))),
        ("test", Box::new(|| cargo_step(&["test", "--workspace"]))),
        ("wasm", Box::new(cmd_wasm)),
    ];
    let mut done = Vec::new();
    for (name, f) in &steps {
        eprintln!("\n=== ci: {name} ===");
        if let Err(e) = f() {
            println!("\nCI summary:");
            for d in &done {
                println!("  ok    {d}");
            }
            println!("  FAIL  {name}: {e}");
            for (n, _) in steps.iter().skip(done.len().saturating_add(1)) {
                println!("  skip  {n}");
            }
            return Err(format!("ci failed at `{name}`"));
        }
        done.push(*name);
    }
    println!("\nCI summary: all {} steps passed ({})", done.len(), done.join(", "));
    Ok(())
}
