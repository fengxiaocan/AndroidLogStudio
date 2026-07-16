use crate::log_entry::{LogEntry, LogLevel};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterTerm {
    Package(String),
    Tag(String),
    Level(LogLevel),
    /// Explicit "no levels selected" from the UI checkbox group.
    NoLevels,
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
        if self.terms.iter().any(|term| matches!(term, FilterTerm::NoLevels)) {
            return false;
        }

        // Level tokens OR together (checkbox multi-select); every other term ANDs.
        let levels: Vec<LogLevel> = self
            .terms
            .iter()
            .filter_map(|term| match term {
                FilterTerm::Level(level) => Some(*level),
                _ => None,
            })
            .collect();

        if !levels.is_empty() && !levels.iter().any(|level| &entry.level == level) {
            return false;
        }

        self.terms.iter().all(|term| match term {
            FilterTerm::Package(value) => entry.package_name.as_deref().unwrap_or("").contains(value),
            FilterTerm::Tag(value) => entry.tag.contains(value),
            FilterTerm::Level(_) | FilterTerm::NoLevels => true, // handled above
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
        "level" if value.eq_ignore_ascii_case("none") => Some(FilterTerm::NoLevels),
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

    #[test]
    fn multiple_level_terms_match_any_selected_level() {
        // UI sends one level: token per checked checkbox; those must OR together.
        let query = FilterQuery::parse("level:warn level:error");
        assert!(query.matches(&entry())); // Error

        let mut warn = entry();
        warn.level = LogLevel::Warn;
        assert!(query.matches(&warn));

        let mut info = entry();
        info.level = LogLevel::Info;
        assert!(!query.matches(&info));
    }

    #[test]
    fn package_and_level_still_and_with_or_levels() {
        let query = FilterQuery::parse("package:example level:warn level:error");
        assert!(query.matches(&entry()));

        let mut other = entry();
        other.package_name = Some("com.other".to_string());
        assert!(!query.matches(&other));
    }

    #[test]
    fn level_none_matches_nothing() {
        let query = FilterQuery::parse("level:none");
        assert!(!query.matches(&entry()));
    }
}
