use crate::log_entry::{LogEntry, LogLevel, StatisticsSnapshot};

#[derive(Debug, Default)]
pub struct Statistics {
    errors: u64,
    warnings: u64,
    hidden: u64,
    total: u64,
}

impl Statistics {
    pub fn observe(&mut self, entry: &LogEntry) {
        self.total += 1;
        if entry.hidden {
            self.hidden += 1;
        }
        match entry.level {
            LogLevel::Error | LogLevel::Assert => self.errors += 1,
            LogLevel::Warn => self.warnings += 1,
            _ => {}
        }
    }

    pub fn set_hidden(&mut self, hidden: u64) {
        self.hidden = hidden;
    }

    pub fn snapshot(&self) -> StatisticsSnapshot {
        StatisticsSnapshot {
            errors: self.errors,
            warnings: self.warnings,
            logs_per_second: 0,
            memory_bytes: self.total * 256,
            hidden: self.hidden,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(level: LogLevel) -> LogEntry {
        LogEntry {
            seq: 1,
            timestamp: 0,
            date: String::new(),
            time: String::new(),
            pid: 0,
            tid: 0,
            level,
            tag: String::new(),
            message: String::new(),
            package_name: None,
            foreground: None,
            background: None,
            hidden: false,
            bookmarked: false,
        }
    }

    #[test]
    fn counts_errors_and_warnings() {
        let mut stats = Statistics::default();
        stats.observe(&entry(LogLevel::Error));
        stats.observe(&entry(LogLevel::Warn));

        let snapshot = stats.snapshot();
        assert_eq!(snapshot.errors, 1);
        assert_eq!(snapshot.warnings, 1);
    }
}
