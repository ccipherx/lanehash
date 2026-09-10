// Stamp the binary with the commit it was built from (methodology rule 1).
use std::process::Command;
fn main() {
    let out = |args: &[&str]| Command::new("git").args(args).output().ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
    let hash = out(&["rev-parse", "--short=10", "HEAD"]).unwrap_or_else(|| "nogit".into());
    let dirty = out(&["status", "--porcelain", "--untracked-files=no"]).map_or(false, |s| !s.is_empty());
    println!("cargo:rustc-env=BENCH_COMMIT={hash}{}", if dirty { "-dirty" } else { "" });
    println!("cargo:rerun-if-changed=../.git/HEAD");
    println!("cargo:rerun-if-changed=../.git/index");
}
