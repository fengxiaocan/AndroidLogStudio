use crate::log_entry::{LogEntry, LogLevel};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterTerm {
    Package(String),
    Tag(String),
    Level(LogLevel),
    Pid(u32),
    Text(String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FilterQuery {
    terms: Vec<FilterTerm>,
}

impl FilterQuery {
    pub fn parse(input: &str) -> Self {
        let terms = input
            .split_whitespace()
            .filter_map(parse_term)
            .collect();
        Self { terms }
    }

    pub fn matches(&self, entry: &LogEntry) -> bool {
        self.terms.iter().all(|term| match term {
            FilterTerm::Package(value) => entry.package_name.as_deref().unwrap_or("").contains(value),
            FilterTerm::Tag(value) => entry.tag.contains(value),
            FilterTerm::Level(value) => &entry.level == value,
            FilterTerm::Pid(value) => entry.pid == *value,
            FilterTerm::Text(value) => entry.message.contains(value),
        })
    }
}

fn parse_term(raw: &str) -> Option<FilterTerm> {
    let (key, value) = raw.split_once(':')?;
    match key {
        "package" => Some(FilterTerm::Package(value.to_string())),
        "tag" => Some(FilterTerm::Tag(value.to_string())),
        "level" => parse_level(value).map(FilterTerm::Level),
        "pid" => value.parse().ok().map(FilterTerm::Pid),
        "text" => Some(FilterTerm::Text(value.to_string())),
        _ => None,
    }
}

fn parse_level(value: &str) -> Option<LogLevel> {
    match value.to_ascii_lowercase().as_str() {
        "v" | "verbose" => Some(LogLevel::Verbose),
        "d" | "debug" => Some(LogLevel::Debug),
        "i" | "info" => Some(LogLevel::Info),
        "w" | "warn" => Some(LogLevel::Warn),
        "e" | "error" => Some(LogLevel::Error),
        "a" | "assert" => Some(LogLevel::Assert),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry() -> LogEntry {
        LogEntry {
            seq: 1,
            timestamp: 0,
            date: "07-04".to_string(),
            time: "12:00:00.000".to_string(),
            pid: 1234,
            tid: 5678,
            level: LogLevel::Error,
            tag: "ActivityManager".to_string(),
            message: "Process crashed".to_string(),
            package_name: Some("com.example".to_string()),
            foreground: None,
            background: None,
            hidden: false,
            bookmarked: false,
        }
    }

    #[test]
    fn matches_basic_and_query() {
        let query = FilterQuery::parse("package:example level:error text:crashed");
        assert!(query.matches(&entry()));
    }

    #[test]
    fn rejects_non_matching_query() {
        let query = FilterQuery::parse("tag:SurfaceFlinger");
        assert!(!query.matches(&entry()));
    }
}
