use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Maps process IDs to package/process names, refreshed from `adb shell ps`.
/// Android Studio Logcat uses the same PID→process/package enrichment path.
#[derive(Debug, Clone)]
pub struct PidCache {
    map: HashMap<u32, String>,
    last_refresh: Option<Instant>,
    refresh_interval: Duration,
}

impl Default for PidCache {
    fn default() -> Self {
        Self::new(Duration::from_secs(5))
    }
}

impl PidCache {
    pub fn new(refresh_interval: Duration) -> Self {
        Self {
            map: HashMap::new(),
            last_refresh: None,
            refresh_interval,
        }
    }

    pub fn resolve(&self, pid: u32) -> Option<&str> {
        self.map.get(&pid).map(String::as_str)
    }

    pub fn insert(&mut self, pid: u32, name: impl Into<String>) {
        let name = name.into();
        if !name.is_empty() {
            self.map.insert(pid, name);
        }
    }

    pub fn needs_refresh(&self) -> bool {
        match self.last_refresh {
            None => true,
            Some(at) => at.elapsed() >= self.refresh_interval,
        }
    }

    pub fn mark_refreshed(&mut self) {
        self.last_refresh = Some(Instant::now());
    }

    /// Parse `ps -A -o PID,NAME` (or similar) output into the cache.
    /// Replaces the entire map so exited processes do not linger forever.
    pub fn apply_ps_output(&mut self, output: &str) {
        let mut next = HashMap::new();
        for line in output.lines() {
            if let Some((pid, name)) = parse_ps_line(line) {
                next.insert(pid, name);
            }
        }
        if !next.is_empty() {
            self.map = next;
            self.mark_refreshed();
        }
    }

}

fn parse_ps_line(line: &str) -> Option<(u32, String)> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Header line: "PID NAME" / "USER PID ..."
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("pid") || lower.starts_with("user") {
        return None;
    }

    let mut parts = trimmed.split_whitespace();
    let first = parts.next()?;
    // Formats:
    //   "1234 com.example.app"
    //   "u0_a123 1234 com.example.app" (USER PID NAME)
    let (pid_token, name_start) = if first.chars().all(|c| c.is_ascii_digit()) {
        (first, parts.next()?)
    } else {
        let pid = parts.next()?;
        let name = parts.next()?;
        (pid, name)
    };

    let pid: u32 = pid_token.parse().ok()?;
    let mut name = name_start.to_string();
    // Join remaining tokens for names with spaces (rare for packages).
    for part in parts {
        name.push(' ');
        name.push_str(part);
    }

    // Kernel threads look like "[kthreadd]"; keep them but package filter
    // rarely matches. Empty names are useless.
    if name.is_empty() {
        return None;
    }

    Some((pid, name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_pid_name_output() {
        let mut cache = PidCache::default();
        cache.apply_ps_output(
            "  PID NAME\n\
             1 init\n\
             1234 com.example.app\n\
             5678 system_server\n",
        );

        assert_eq!(cache.resolve(1234), Some("com.example.app"));
        assert_eq!(cache.resolve(5678), Some("system_server"));
        assert_eq!(cache.resolve(1), Some("init"));
        assert_eq!(cache.resolve(9999), None);
    }

    #[test]
    fn parses_user_pid_name_output() {
        let mut cache = PidCache::default();
        cache.apply_ps_output(
            "USER           PID NAME\n\
             u0_a100       4321 com.android.systemui\n",
        );

        assert_eq!(cache.resolve(4321), Some("com.android.systemui"));
    }

    #[test]
    fn needs_refresh_before_first_apply() {
        let cache = PidCache::new(Duration::from_secs(60));
        assert!(cache.needs_refresh());
    }

    #[test]
    fn insert_and_resolve() {
        let mut cache = PidCache::default();
        cache.insert(42, "com.demo");
        assert_eq!(cache.resolve(42), Some("com.demo"));
    }
}
