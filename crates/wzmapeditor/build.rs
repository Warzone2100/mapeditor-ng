//! Bake the current git commit hash into the binary as `WZ_GIT_HASH`.
//!
//! The web build shows it on the dev deployment so the live commit is
//! identifiable at a glance (production hides it).

use std::path::Path;
use std::process::Command;

fn main() {
    let hash = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        // CI runners may build without a usable git checkout; GitHub sets this.
        .or_else(|| {
            std::env::var("GITHUB_SHA")
                .ok()
                .map(|s| s.chars().take(7).collect())
        })
        .unwrap_or_else(|| "unknown".to_owned());
    println!("cargo:rustc-env=WZ_GIT_HASH={hash}");

    // Re-run when HEAD moves so incremental local builds don't bake a stale
    // hash; `logs/HEAD` is appended on every commit/checkout/reset. CI builds
    // fresh, so it always re-runs there regardless.
    for rel in ["../../.git/HEAD", "../../.git/logs/HEAD"] {
        if Path::new(rel).exists() {
            println!("cargo:rerun-if-changed={rel}");
        }
    }
}
