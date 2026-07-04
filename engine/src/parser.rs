use crate::log_entry::{LogEntry, LogLevel};
use regex::Regex;
use std::sync::OnceLock;

const MAX_MESSAGE_BYTES: usize = 10 * 1024;

pub fn parse_threadtime_line(seq: u64, line: &str) -> Option<LogEntry> {
    let Some(captures) = threadtime_regex().captures(line) else {
        return Some(unknown_entry(seq, line));
    };

    let Some(pid) = captures
        .name("pid")
        .and_then(|pid| pid.as_str().parse().ok())
    else {
        return Some(unknown_entry(seq, line));
    };
    let Some(tid) = captures
        .name("tid")
        .and_then(|tid| tid.as_str().parse().ok())
    else {
        return Some(unknown_entry(seq, line));
    };

    Some(LogEntry {
        seq,
        timestamp: 0,
        date: captures
            .name("date")
            .map(|date| date.as_str().to_owned())
            .unwrap_or_default(),
        time: captures
            .name("time")
            .map(|time| time.as_str().to_owned())
            .unwrap_or_default(),
        pid,
        tid,
        level: captures
            .name("level")
            .map(|level| parse_level(level.as_str()))
            .unwrap_or(LogLevel::Unknown),
        tag: captures
            .name("tag")
            .map(|tag| tag.as_str().to_owned())
            .unwrap_or_default(),
        message: truncate_message(
            captures
                .name("message")
                .map(|message| message.as_str())
                .unwrap_or_default(),
        ),
        package_name: None,
        foreground: None,
        background: None,
        hidden: false,
        bookmarked: false,
    })
}

fn threadtime_regex() -> &'static Regex {
    static THREADTIME_REGEX: OnceLock<Regex> = OnceLock::new();
    THREADTIME_REGEX.get_or_init(|| {
        Regex::new(
            r"^(?P<date>\d{2}-\d{2})\s+(?P<time>\d{2}:\d{2}:\d{2}\.\d{3})\s+(?P<pid>\d+)\s+(?P<tid>\d+)\s+(?P<level>\S)\s+(?P<tag>[^:]*):\s?(?P<message>.*)$",
        )
        .expect("threadtime regex should compile")
    })
}

fn parse_level(level: &str) -> LogLevel {
    match level {
        "V" => LogLevel::Verbose,
        "D" => LogLevel::Debug,
        "I" => LogLevel::Info,
        "W" => LogLevel::Warn,
        "E" => LogLevel::Error,
        "A" | "F" => LogLevel::Assert,
        _ => LogLevel::Unknown,
    }
}

fn unknown_entry(seq: u64, line: &str) -> LogEntry {
    LogEntry {
        seq,
        timestamp: 0,
        date: String::new(),
        time: String::new(),
        pid: 0,
        tid: 0,
        level: LogLevel::Unknown,
        tag: String::new(),
        message: truncate_message(line),
        package_name: None,
        foreground: None,
        background: None,
        hidden: false,
        bookmarked: false,
    }
}

fn truncate_message(message: &str) -> String {
    if message.len() <= MAX_MESSAGE_BYTES {
        return message.to_owned();
    }

    let mut end = MAX_MESSAGE_BYTES;
    while !message.is_char_boundary(end) {
        end -= 1;
    }

    message[..end].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::log_entry::LogLevel;

    #[test]
    fn parses_standard_threadtime_line() {
        let entry = parse_threadtime_line(
            42,
            "07-04 12:34:56.789  1234  5678 I ActivityManager: Start proc com.example",
        )
        .expect("threadtime line should parse");

        assert_eq!(entry.seq, 42);
        assert_eq!(entry.timestamp, 0);
        assert_eq!(entry.date, "07-04");
        assert_eq!(entry.time, "12:34:56.789");
        assert_eq!(entry.pid, 1234);
        assert_eq!(entry.tid, 5678);
        assert_eq!(entry.level, LogLevel::Info);
        assert_eq!(entry.tag, "ActivityManager");
        assert_eq!(entry.message, "Start proc com.example");
        assert_eq!(entry.package_name, None);
        assert_eq!(entry.foreground, None);
        assert_eq!(entry.background, None);
        assert!(!entry.hidden);
        assert!(!entry.bookmarked);
    }

    #[test]
    fn returns_unknown_entry_for_unparseable_line() {
        let raw_line = "this is not a threadtime line";
        let entry = parse_threadtime_line(7, raw_line).expect("raw line should be preserved");

        assert_eq!(entry.seq, 7);
        assert_eq!(entry.timestamp, 0);
        assert_eq!(entry.date, "");
        assert_eq!(entry.time, "");
        assert_eq!(entry.pid, 0);
        assert_eq!(entry.tid, 0);
        assert_eq!(entry.level, LogLevel::Unknown);
        assert_eq!(entry.tag, "");
        assert_eq!(entry.message, raw_line);
        assert_eq!(entry.package_name, None);
        assert_eq!(entry.foreground, None);
        assert_eq!(entry.background, None);
        assert!(!entry.hidden);
        assert!(!entry.bookmarked);
    }

    #[test]
    fn truncates_large_message_to_ten_kib() {
        let large_message = "🙂".repeat(3_000);
        let line = format!("07-04 12:34:56.789  1234  5678 D Tag: {large_message}");

        let entry = parse_threadtime_line(9, &line).expect("threadtime line should parse");

        assert_eq!(entry.message.len(), 10 * 1024);
        assert_eq!(entry.message, "🙂".repeat((10 * 1024) / "🙂".len()));
    }
}
