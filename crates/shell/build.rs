//! Build script: emit `VICTRON_CONTROLLER_GIT_SHA` as a cargo
//! `rustc-env` so the shell's `option_env!` macro picks up the
//! current commit at compile time.
//!
//! Resolution order:
//! 1. The `VICTRON_CONTROLLER_GIT_SHA` env var if already set (so Nix
//!    derivations can pin the SHA without invoking git in the
//!    sandbox).
//! 2. `git rev-parse --short=12 HEAD` in the source tree.
//! 3. Nothing — `option_env!` then resolves to `None` and the
//!    dashboard's version-reload feature degrades to a no-op.
//!
//! The script also tells cargo to rerun whenever the checkout's HEAD
//! moves so a `cargo build` after `git checkout other-branch` picks
//! up the new SHA without a `cargo clean`.

use std::process::Command;

fn main() {
    let sha = std::env::var("VICTRON_CONTROLLER_GIT_SHA")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(git_sha);
    if let Some(sha) = sha {
        println!("cargo:rustc-env=VICTRON_CONTROLLER_GIT_SHA={sha}");
    }

    // Rerun whenever the working tree's commit moves. `.git/HEAD`
    // changes on branch switch; the ref it points at changes on
    // commit. We watch both via the repo root — cargo follows
    // symlinks so this catches worktrees too.
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-env-changed=VICTRON_CONTROLLER_GIT_SHA");
}

fn git_sha() -> Option<String> {
    let out = Command::new("git")
        .args(["rev-parse", "--short=12", "HEAD"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?;
    let trimmed = s.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
