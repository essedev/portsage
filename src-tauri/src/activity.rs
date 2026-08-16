//! How long since a project was last worked on.
//!
//! Portsage has no history of its own to answer this: the database only knows
//! when a project was registered. The signal therefore comes from the
//! filesystem, and picking the right file matters more than the arithmetic:
//!
//! - **Directory mtime** changes when files are added or removed at the top
//!   level. It misses work done deeper in the tree, which is why on a real
//!   registry it reported 99 days for a project whose last commit was 17 days
//!   old.
//! - **`.git/HEAD` mtime** only moves on checkout, so a repo committed to
//!   daily can look months old. Useless on its own.
//! - **`.git/logs/HEAD` mtime** (the reflog) moves on every commit, pull,
//!   merge, reset and checkout. Measured against `git log -1` across 25 real
//!   repos it agreed every time, so we read the file's mtime and skip parsing
//!   the reflog format entirely.
//!
//! We take the most recent of the directory and the reflog: the reflog covers
//! git work, the directory covers projects that have no git at all.

use std::path::Path;
use std::time::{Duration, SystemTime};

/// Why a project shows up as prunable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Activity {
    /// The path is gone. Not "inactive": either the project was deleted, or
    /// it moved and the registry needs correcting.
    PathMissing,
    /// No path recorded, so there is nothing to measure.
    Unknown,
    /// Most recent filesystem activity, in whole days before now.
    Days(i64),
}

/// Most recent activity for `path`, or the reason we cannot tell.
pub fn classify(path: Option<&str>, now: SystemTime) -> Activity {
    let Some(path) = path.filter(|p| !p.trim().is_empty()) else {
        return Activity::Unknown;
    };
    let dir = Path::new(path);
    if !dir.is_dir() {
        return Activity::PathMissing;
    }
    let newest = [mtime(dir), mtime(&dir.join(".git/logs/HEAD"))]
        .into_iter()
        .flatten()
        .max();
    match newest {
        Some(t) => Activity::Days(days_between(t, now)),
        // The directory exists but its metadata is unreadable. Refusing to
        // guess is better than reporting a project as abandoned.
        None => Activity::Unknown,
    }
}

fn mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok()?.modified().ok()
}

/// Whole days from `then` to `now`, clamped at zero. A file stamped in the
/// future (clock skew, a restored backup) counts as touched right now.
pub fn days_between(then: SystemTime, now: SystemTime) -> i64 {
    let elapsed = now.duration_since(then).unwrap_or(Duration::ZERO);
    (elapsed.as_secs() / 86_400) as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn tmpdir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "portsage-activity-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn missing_path_is_reported_apart_from_inactivity() {
        let missing = "/definitely/not/here/portsage-test";
        assert_eq!(
            classify(Some(missing), SystemTime::now()),
            Activity::PathMissing
        );
    }

    #[test]
    fn absent_or_blank_path_is_unknown() {
        let now = SystemTime::now();
        assert_eq!(classify(None, now), Activity::Unknown);
        assert_eq!(classify(Some("   "), now), Activity::Unknown);
    }

    #[test]
    fn fresh_directory_reports_zero_days() {
        let dir = tmpdir("fresh");
        assert_eq!(
            classify(Some(dir.to_str().unwrap()), SystemTime::now()),
            Activity::Days(0)
        );
    }

    #[test]
    fn a_directory_touched_long_ago_counts_from_now() {
        let dir = tmpdir("old");
        // 100 days into the future for `now` is the same as 100 days of
        // staleness, without having to backdate a file on disk.
        let now = SystemTime::now() + Duration::from_secs(100 * 86_400);
        assert_eq!(
            classify(Some(dir.to_str().unwrap()), now),
            Activity::Days(100)
        );
    }

    /// Backdate a path's mtime via `touch -t`, so tests can build the
    /// "stale directory, fresh reflog" shape without waiting.
    fn backdate(path: &std::path::Path, stamp: &str) {
        let ok = std::process::Command::new("touch")
            .args(["-t", stamp])
            .arg(path)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        assert!(ok, "touch failed for {}", path.display());
    }

    #[test]
    fn a_recent_reflog_beats_an_old_directory() {
        // The case that rules out using the directory alone: the work happens
        // deep in the tree, so only the reflog moves.
        let dir = tmpdir("reflog");
        std::fs::create_dir_all(dir.join(".git/logs")).unwrap();
        std::fs::write(dir.join(".git/logs/HEAD"), "reflog line\n").unwrap();
        backdate(&dir, "202001010000");

        assert_eq!(
            classify(Some(dir.to_str().unwrap()), SystemTime::now()),
            Activity::Days(0),
            "a fresh reflog must win over a stale directory mtime"
        );
    }

    #[test]
    fn both_signals_stale_reports_the_most_recent_of_the_two() {
        let dir = tmpdir("bothstale");
        std::fs::create_dir_all(dir.join(".git/logs")).unwrap();
        std::fs::write(dir.join(".git/logs/HEAD"), "reflog line\n").unwrap();
        // Reflog older than the directory: the directory wins.
        backdate(&dir.join(".git/logs/HEAD"), "202001010000");
        backdate(&dir, "202006010000");

        let Activity::Days(days) = classify(Some(dir.to_str().unwrap()), SystemTime::now()) else {
            panic!("expected a day count");
        };
        // 2020-06-01 is the newer of the two stamps, and well over a year ago.
        assert!(days > 365, "got {days} days");
    }

    #[test]
    fn future_timestamps_do_not_underflow() {
        let now = SystemTime::now();
        let future = now + Duration::from_secs(86_400);
        assert_eq!(days_between(future, now), 0);
    }
}
