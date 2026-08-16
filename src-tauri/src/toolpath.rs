//! Locating the external binaries Portsage shells out to.
//!
//! A macOS app bundle started by launchd (Finder, login item, tray) inherits
//! `PATH=/usr/bin:/bin:/usr/sbin:/sbin` - `launchctl getenv PATH` is unset on
//! a stock system. A build started from a terminal inherits the user's PATH
//! instead, which frequently omits `/usr/sbin` and never contains
//! `/usr/local/bin` by default. Either way, `Command::new("lsof")` can fail
//! with ENOENT while the same command works fine in the user's shell, and the
//! failure is silent: an empty scan looks exactly like "nothing is listening".
//!
//! So every external tool is resolved explicitly: an env override, then PATH,
//! then the known absolute locations.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// True when a path exists and carries at least one executable bit.
pub fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path)
            .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

/// Look `name` up in a `PATH`-shaped string. Takes the variable as an
/// argument rather than reading the environment so tests can drive it
/// without mutating process-global state.
pub fn find_in_path_var(path_var: &str, name: &str) -> Option<PathBuf> {
    path_var
        .split(':')
        .filter(|dir| !dir.is_empty())
        .map(|dir| Path::new(dir).join(name))
        .find(|candidate| is_executable(candidate))
}

/// Resolve an external tool: `env_var` override first (when given), then
/// `PATH`, then `candidates` in order. Deliberately uncached - the app can
/// sit in the tray for days, and a tool installed after launch should not
/// need a restart to be found. The cost is a handful of `stat` calls on
/// paths that run at most once per user action.
pub fn resolve(name: &str, env_var: Option<&str>, candidates: &[&str]) -> Option<PathBuf> {
    if let Some(var) = env_var {
        if let Some(explicit) = std::env::var_os(var) {
            let p = PathBuf::from(explicit);
            if is_executable(&p) {
                return Some(p);
            }
        }
    }
    if let Some(path_var) = std::env::var_os("PATH") {
        if let Some(found) = find_in_path_var(&path_var.to_string_lossy(), name) {
            return Some(found);
        }
    }
    candidates
        .iter()
        .map(PathBuf::from)
        .find(|c| is_executable(c))
}

/// Resolve a tool, falling back to the bare name when nothing matches. Used
/// by callers that would rather let the OS produce a spawn error than skip
/// the work entirely.
pub fn resolve_or_bare(name: &str, candidates: &[&str]) -> PathBuf {
    resolve(name, None, candidates).unwrap_or_else(|| PathBuf::from(name))
}

/// Convenience for building a `Command` against a resolved tool.
pub fn command(program: impl AsRef<OsStr>) -> std::process::Command {
    std::process::Command::new(program)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn stub_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("portsage-toolpath-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_stub(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, "#!/bin/sh\nexit 0\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    #[test]
    fn find_in_path_var_returns_first_executable_hit() {
        let first = stub_dir("first");
        let second = stub_dir("second");
        let expected = write_stub(&first, "toolstub");
        write_stub(&second, "toolstub");
        let path_var = format!("{}:{}", first.display(), second.display());
        assert_eq!(find_in_path_var(&path_var, "toolstub"), Some(expected));
    }

    #[test]
    fn find_in_path_var_skips_non_executable_and_missing_entries() {
        let dir = stub_dir("noexec");
        // A file without the executable bit is what a stray config or an
        // un-chmodded wrapper looks like; picking it would fail at spawn.
        std::fs::write(dir.join("toolstub"), "not a binary").unwrap();
        let path_var = format!("/nonexistent-portsage-dir::{}", dir.display());
        assert_eq!(find_in_path_var(&path_var, "toolstub"), None);
    }

    #[test]
    fn resolve_falls_back_to_the_candidate_list() {
        let dir = stub_dir("candidates");
        let stub = write_stub(&dir, "candidatestub");
        let candidates = [stub.to_str().unwrap()];
        // The name is not in PATH, so only the absolute candidate can match.
        assert_eq!(
            resolve("definitely-not-on-path-portsage", None, &candidates),
            Some(stub)
        );
    }

    #[test]
    fn resolve_returns_none_when_nothing_matches() {
        assert_eq!(
            resolve(
                "definitely-not-on-path-portsage",
                None,
                &["/nonexistent/portsage/tool"]
            ),
            None
        );
    }

    #[test]
    fn resolve_or_bare_hands_back_the_name_for_the_os_to_reject() {
        assert_eq!(
            resolve_or_bare("definitely-not-on-path-portsage", &[]),
            PathBuf::from("definitely-not-on-path-portsage")
        );
    }
}
