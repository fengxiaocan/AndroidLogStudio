use crate::log_entry::{LogEntry, LogLevel};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterTerm {
    Package(String),
    Tag(String),
    ExcludePackage(String),
    ExcludeTag(String),
    Level(LogLevel),
    /// Explicit "no levels selected" from the UI checkbox group.
    NoLevels,
    Pid(u32),
    Text(String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FilterQuery {
    terms: Vec<FilterTerm>,
    case_insensitive: bool,
}

impl FilterQuery {
    pub fn parse(input: &str) -> Self {
        let mut terms: Vec<FilterTerm> = Vec::new();
        let mut case_insensitive = false;

        for raw in input.split_whitespace() {
            let lower = raw.to_ascii_lowercase();
            if lower == "case:insensitive" || lower == "case:ignore" || lower == "ci:true" || lower == "ci" {
                case_insensitive = true;
                continue;
            }

            if let Some(parsed) = parse_term(raw) {
                terms.extend(parsed);
            }
        }
        Self { terms, case_insensitive }
    }

    pub fn is_case_insensitive(&self) -> bool {
        self.case_insensitive
    }

    pub fn matches(&self, entry: &LogEntry) -> bool {
        if self.terms.iter().any(|term| matches!(term, FilterTerm::NoLevels)) {
            return false;
        }

        let ci = self.case_insensitive;

        // Helper for case-insensitive or sensitive contains
        let matches_str = |haystack: &str, needle: &str| -> bool {
            if ci {
                haystack.to_ascii_lowercase().contains(&needle.to_ascii_lowercase())
            } else {
                haystack.contains(needle)
            }
        };

        // Level tokens OR together (checkbox multi-select)
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

        // Package tokens OR together (support multi-package via | )
        let package_values: Vec<&str> = self
            .terms
            .iter()
            .filter_map(|term| match term {
                FilterTerm::Package(v) => Some(v.as_str()),
                _ => None,
            })
            .collect();

        if !package_values.is_empty() && !package_values.iter().any(|p| matches_str(entry.package_name.as_deref().unwrap_or(""), p)) {
            return false;
        }

        // Negative packages: must NOT match any
        let exclude_packages: Vec<&str> = self
            .terms
            .iter()
            .filter_map(|term| match term {
                FilterTerm::ExcludePackage(v) => Some(v.as_str()),
                _ => None,
            })
            .collect();

        if exclude_packages.iter().any(|p| matches_str(entry.package_name.as_deref().unwrap_or(""), p)) {
            return false;
        }

        // Tag tokens OR together (support multi-tag via multiple `tag:xxx` or | in UI)
        let tag_values: Vec<&str> = self
            .terms
            .iter()
            .filter_map(|term| match term {
                FilterTerm::Tag(v) => Some(v.as_str()),
                _ => None,
            })
            .collect();

        if !tag_values.is_empty() && !tag_values.iter().any(|t| matches_str(&entry.tag, t)) {
            return false;
        }

        // Negative tags: must NOT match any
        let exclude_tags: Vec<&str> = self
            .terms
            .iter()
            .filter_map(|term| match term {
                FilterTerm::ExcludeTag(v) => Some(v.as_str()),
                _ => None,
            })
            .collect();

        if exclude_tags.iter().any(|t| matches_str(&entry.tag, t)) {
            return false;
        }

        // All other terms are ANDed
        self.terms.iter().all(|term| match term {
            FilterTerm::Package(_) | FilterTerm::ExcludePackage(_) => true,
            FilterTerm::Tag(_) | FilterTerm::ExcludeTag(_) => true, // handled above
            FilterTerm::Level(_) | FilterTerm::NoLevels => true,
            FilterTerm::Pid(value) => entry.pid == *value,
            FilterTerm::Text(value) => matches_str(&entry.message, value),
        })
    }
}

fn parse_term(raw: &str) -> Option<Vec<FilterTerm>> {
    // Support negation prefix: -package:foo or -tag:bar
    let (is_negated, term) = if raw.starts_with('-') {
        (true, &raw[1..])
    } else {
        (false, raw)
    };

    let (key, value) = term.split_once(':')?;

    let make = |s: String| -> FilterTerm {
        if is_negated {
            // For now only support exclude for package/tag
            match key {
                "package" => FilterTerm::ExcludePackage(s),
                "tag" => FilterTerm::ExcludeTag(s),
                _ => FilterTerm::Text(s), // fallback
            }
        } else {
            match key {
                "package" => FilterTerm::Package(s),
                "tag" => FilterTerm::Tag(s),
                _ => unreachable!(),
            }
        }
    };

    match key {
        "package" => {
            let parts: Vec<FilterTerm> = value
                .split('|')
                .map(|p| p.trim())
                .filter(|p| !p.is_empty())
                .map(|p| {
                    // allow - inside split too
                    let (neg, val) = if p.starts_with('-') { (true, &p[1..]) } else { (false, p) };
                    let effective_neg = is_negated || neg;
                    if effective_neg {
                        FilterTerm::ExcludePackage(val.to_string())
                    } else {
                        FilterTerm::Package(val.to_string())
                    }
                })
                .collect();
            if parts.is_empty() { None } else { Some(parts) }
        }
        "tag" => {
            let parts: Vec<FilterTerm> = value
                .split('|')
                .map(|p| p.trim())
                .filter(|p| !p.is_empty())
                .map(|p| {
                    let (neg, val) = if p.starts_with('-') { (true, &p[1..]) } else { (false, p) };
                    let effective_neg = is_negated || neg;
                    if effective_neg {
                        FilterTerm::ExcludeTag(val.to_string())
                    } else {
                        FilterTerm::Tag(val.to_string())
                    }
                })
                .collect();
            if parts.is_empty() { None } else { Some(parts) }
        }
        "level" if value.eq_ignore_ascii_case("none") => Some(vec![FilterTerm::NoLevels]),
        "level" => parse_level(value).map(|l| vec![FilterTerm::Level(l)]),
        "pid" => value.parse().ok().map(|p| vec![FilterTerm::Pid(p)]),
        "text" => Some(vec![FilterTerm::Text(value.to_string())]),
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

    #[test]
    fn multiple_tag_terms_or_together() {
        // Multiple tag: terms (from UI | split or direct) should OR
        let query = FilterQuery::parse("tag:SurfaceFlinger tag:ActivityManager");
        assert!(query.matches(&entry())); // matches ActivityManager

        let mut other = entry();
        other.tag = "WindowManager".to_string();
        assert!(!query.matches(&other));
    }

    #[test]
    fn tag_with_pipe_syntax() {
        // Direct support for tag:foo|bar|baz syntax
        let query = FilterQuery::parse("tag:SurfaceFlinger|ActivityManager");
        assert!(query.matches(&entry()));

        let mut other = entry();
        other.tag = "Choreographer".to_string();
        assert!(!query.matches(&other));
    }

    #[test]
    fn tag_or_with_other_and_terms() {
        let query = FilterQuery::parse("package:example tag:foo|ActivityManager");
        assert!(query.matches(&entry()));

        let mut no_pkg = entry();
        no_pkg.package_name = Some("com.other".to_string());
        assert!(!query.matches(&no_pkg)); // package still AND
    }

    #[test]
    fn package_multi_or() {
        let query = FilterQuery::parse("package:com.other|com.example");
        assert!(query.matches(&entry()));
    }

    #[test]
    fn negation_exclude_tag() {
        let query = FilterQuery::parse("tag:ActivityManager -tag:SurfaceFlinger");
        assert!(query.matches(&entry()));

        let mut bad = entry();
        bad.tag = "SurfaceFlinger".to_string();
        assert!(!query.matches(&bad));
    }

    #[test]
    fn case_insensitive() {
        let mut entry_ci = entry();
        entry_ci.tag = "activitymanager".to_string();
        entry_ci.package_name = Some("Com.Example".to_string());
        entry_ci.message = "process CRASHED".to_string();

        let query = FilterQuery::parse("tag:activitymanager package:com.example text:CRASHED case:insensitive");
        assert!(query.matches(&entry_ci));

        // without ci should fail for mismatched case
        let query_sensitive = FilterQuery::parse("tag:activitymanager package:com.example text:CRASHED");
        assert!(!query_sensitive.matches(&entry_ci));
    }

    #[test]
    fn mixed_negation_and_or() {
        let query = FilterQuery::parse("tag:foo|ActivityManager -package:com.bad");
        assert!(query.matches(&entry()));

        let mut bad_pkg = entry();
        bad_pkg.package_name = Some("com.bad".to_string());
        assert!(!query.matches(&bad_pkg));
    }
}
