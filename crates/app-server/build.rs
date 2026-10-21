//! Embeds the identity of the source tree the binary was built from.
//!
//! A runtime answer is only reproducible if the code that produced it is
//! known. The commit is baked in at build time so `/api/diagnostics` can
//! name it without shelling out to git at runtime (which would fail in a
//! packaged install and would race with the working tree).
//!
//! The script never fails the build: outside a git checkout it emits
//! `unknown`, which the diagnostics surface reports honestly.

use std::process::Command;

/// Runs a git command in the workspace root and returns its trimmed stdout.
fn git(args: &[&str]) -> Option<String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)?
        .to_path_buf();
    let output = Command::new("git")
        .args(args)
        .current_dir(&root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn main() {
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/index");
    println!("cargo:rerun-if-env-changed=SEVEN_DEGREES_COMMIT");

    let commit = std::env::var("SEVEN_DEGREES_COMMIT")
        .ok()
        .or_else(|| git(&["rev-parse", "HEAD"]))
        .unwrap_or_else(|| "unknown".to_string());
    // A dirty tree means the binary does not match the commit it names.
    let dirty = git(&["status", "--porcelain"]).is_some_and(|status| !status.is_empty());
    println!("cargo:rustc-env=SEVEN_DEGREES_COMMIT={commit}");
    println!("cargo:rustc-env=SEVEN_DEGREES_DIRTY={dirty}");
}
