//! host.rs - talking to the n12-monterey checkout, which owns device bring-up.
//!
//! The control panel does not build the CoreAudio device itself: n12-monterey's
//! `n12_agent` does. And the driver only applies a new sample rate when the
//! device *starts*, so changing the rate means restarting the device -
//! `n12_on.sh --rate=HZ` does exactly that. This module finds that checkout and
//! runs it.

use std::path::PathBuf;
use std::process::Command;

/// Locate n12-monterey's `tools` directory. Tried in order:
/// 1. `$N12_TOOLS` (an explicit path),
/// 2. a checkout **next to this project** (both live under one parent folder),
/// 3. `n12-monterey/tools` relative to the current directory.
pub fn n12_tools_dir() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("N12_TOOLS") {
        let dir = PathBuf::from(p);
        if dir.join("n12_on.sh").exists() {
            return Some(dir);
        }
    }

    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        // .../n12_control_panel/target/<profile>/<bin> -> the projects folder
        let mut dir = exe.as_path();
        for _ in 0..4 {
            match dir.parent() {
                Some(p) => dir = p,
                None => break,
            }
        }
        candidates.push(dir.join("n12-monterey/tools"));
    }
    candidates.push(PathBuf::from("../n12-monterey/tools"));
    candidates.push(PathBuf::from("n12-monterey/tools"));

    candidates.into_iter().find(|c| c.join("n12_on.sh").exists())
}

/// Restart the device at `hz`. Blocks until `n12_on.sh` finishes (about 10-20 s).
///
/// Returns `Err` with a human-readable reason if the checkout is missing or the
/// script fails. Callers that have a UI should run this on a background thread.
pub fn rate_restart(hz: u32) -> Result<(), String> {
    let tools = n12_tools_dir().ok_or_else(|| {
        "could not find n12-monterey's tools (set N12_TOOLS=/path/to/n12-monterey/tools)".to_string()
    })?;
    let script = tools.join("n12_on.sh");

    let status = Command::new("/bin/zsh")
        .arg(&script)
        .arg(format!("--rate={}", hz))
        .status()
        .map_err(|e| format!("could not run {}: {}", script.display(), e))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("{} exited with {}", script.display(), status))
    }
}
