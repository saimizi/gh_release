use crate::errors::Result;
use globset::{Glob, GlobMatcher};
use regex::Regex;

/// Filter type for asset filtering
#[derive(Debug)]
pub enum FilterType {
    /// Substring match (e.g., "linux")
    Substring(String),
    /// Glob pattern (e.g., "*.deb")
    Glob(GlobMatcher),
    /// Regex pattern (e.g., "linux-.*-amd64")
    Regex(Regex),
    /// Exclude pattern (e.g., "!windows")
    Exclude(Box<FilterType>),
}

impl FilterType {
    /// Check if the given name matches this filter
    pub fn matches(&self, name: &str) -> bool {
        match self {
            FilterType::Substring(s) => name.contains(s),
            FilterType::Glob(g) => g.is_match(name),
            FilterType::Regex(r) => r.is_match(name),
            FilterType::Exclude(f) => !f.matches(name),
        }
    }
}

/// Parse a filter string into a FilterType
pub fn parse_filter(s: &str) -> Result<FilterType> {
    // Check for exclude pattern
    if let Some(pattern) = s.strip_prefix('!') {
        return Ok(FilterType::Exclude(Box::new(parse_filter(pattern)?)));
    }

    // Check for regex pattern (contains regex metacharacters) - check before glob
    if s.contains('^')
        || s.contains('$')
        || s.contains(".*")
        || s.contains("\\.")
        || s.contains('(')
        || s.contains('[')
    {
        let regex = Regex::new(s)?;
        return Ok(FilterType::Regex(regex));
    }

    // Check for glob pattern (contains * or ?)
    if s.contains('*') || s.contains('?') {
        let glob = Glob::new(s)?.compile_matcher();
        return Ok(FilterType::Glob(glob));
    }

    // Default to substring match
    Ok(FilterType::Substring(s.to_string()))
}

/// Apply multiple filters to a name
pub fn apply_filters(name: &str, filters: &[FilterType]) -> bool {
    if filters.is_empty() {
        return true;
    }

    // All filters must match (AND logic)
    filters.iter().all(|f| f.matches(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_substring_filter() {
        let filter = parse_filter("linux").unwrap();
        assert!(filter.matches("linux-amd64"));
        assert!(filter.matches("my-linux-app"));
        assert!(!filter.matches("windows-x86"));
    }

    #[test]
    fn test_glob_filter() {
        let filter = parse_filter("*.deb").unwrap();
        assert!(filter.matches("app-1.0.0.deb"));
        assert!(filter.matches("package.deb"));
        assert!(!filter.matches("app.tar.gz"));
    }

    #[test]
    fn test_regex_filter() {
        let filter = parse_filter("linux-.*-amd64").unwrap();
        assert!(filter.matches("linux-musl-amd64"));
        assert!(filter.matches("linux-gnu-amd64"));
        assert!(!filter.matches("linux-arm64"));
    }

    #[test]
    fn test_exclude_filter() {
        let filter = parse_filter("!windows").unwrap();
        assert!(filter.matches("linux-amd64"));
        assert!(!filter.matches("windows-x86"));
        assert!(!filter.matches("app-windows.exe"));
    }

    #[test]
    fn test_apply_multiple_filters() {
        let filters = vec![
            parse_filter("*.deb").unwrap(),
            parse_filter("!test").unwrap(),
        ];

        assert!(apply_filters("app-1.0.0.deb", &filters));
        assert!(!apply_filters("test-1.0.0.deb", &filters));
        assert!(!apply_filters("app-1.0.0.tar.gz", &filters));
    }

    #[test]
    fn test_empty_filters() {
        let filters = vec![];
        assert!(apply_filters("any-file.txt", &filters));
    }
}
