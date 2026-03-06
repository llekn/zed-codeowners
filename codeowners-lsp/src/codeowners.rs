use globset::{Glob, GlobMatcher};
use std::fs;
use std::path::Path;

#[derive(Debug)]
struct Rule {
    matcher: GlobMatcher,
    owners: Vec<String>,
}

#[derive(Debug)]
pub struct Codeowners {
    rules: Vec<Rule>,
}

impl Codeowners {
    /// Find and parse CODEOWNERS from the workspace root.
    /// Searches: .github/CODEOWNERS, CODEOWNERS, docs/CODEOWNERS
    pub fn from_workspace(root: &Path) -> Option<Self> {
        let candidates = [
            root.join(".github/CODEOWNERS"),
            root.join("CODEOWNERS"),
            root.join("docs/CODEOWNERS"),
        ];

        for path in &candidates {
            if let Ok(content) = fs::read_to_string(path) {
                return Some(Self::parse(&content));
            }
        }
        None
    }

    pub fn parse(content: &str) -> Self {
        let mut rules = Vec::new();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 2 {
                continue;
            }

            let pattern = parts[0];
            let owners: Vec<String> = parts[1..].iter().map(|s| s.to_string()).collect();

            if let Some(matcher) = build_matcher(pattern) {
                rules.push(Rule { matcher, owners });
            }
        }

        Codeowners { rules }
    }

    /// Find owners for a file path (relative to workspace root).
    /// Returns the owners from the last matching rule (GitHub precedence).
    pub fn owners_of(&self, file_path: &str) -> Option<&[String]> {
        let mut last_match: Option<&[String]> = None;

        // Normalize: strip leading slash for matching
        let path = file_path.strip_prefix('/').unwrap_or(file_path);

        for rule in &self.rules {
            if rule.matcher.is_match(path) {
                last_match = Some(&rule.owners);
            }
        }

        last_match
    }
}

/// Build a glob matcher from a CODEOWNERS pattern.
///
/// CODEOWNERS patterns follow gitignore-like rules:
/// - `*` matches everything
/// - `*.js` matches files by extension
/// - `/dir/` matches a directory at the root
/// - `dir/` matches a directory at any depth
/// - `docs/*` matches direct children
/// - `**/logs` matches at any depth
fn build_matcher(pattern: &str) -> Option<GlobMatcher> {
    let pat = pattern.strip_prefix('/').unwrap_or(pattern);

    let glob_pattern = if pat.ends_with('/') {
        // Directory pattern: match everything inside it
        format!("{pat}**")
    } else if !pat.contains('/') {
        // No slash: matches at any depth
        format!("**/{pat}")
    } else {
        pat.to_string()
    };

    Glob::new(&glob_pattern).ok().map(|g| g.compile_matcher())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_and_match() {
        let content = r#"
# Global owners
* @global-owner

# Frontend
*.js @frontend-team
*.ts @frontend-team

# Backend
/src/api/ @backend-team

# Docs
docs/* @docs-team
"#;
        let co = Codeowners::parse(content);

        // Global fallback
        assert_eq!(
            co.owners_of("random-file.txt"),
            Some(["@global-owner".to_string()].as_slice())
        );

        // Frontend override
        assert_eq!(
            co.owners_of("app.js"),
            Some(["@frontend-team".to_string()].as_slice())
        );

        // Backend
        assert_eq!(
            co.owners_of("src/api/handler.rs"),
            Some(["@backend-team".to_string()].as_slice())
        );

        // Docs
        assert_eq!(
            co.owners_of("docs/readme.md"),
            Some(["@docs-team".to_string()].as_slice())
        );
    }

    #[test]
    fn test_last_match_wins() {
        let content = r#"
* @fallback
*.rs @rust-team
/src/main.rs @lead-dev
"#;
        let co = Codeowners::parse(content);

        assert_eq!(
            co.owners_of("src/main.rs"),
            Some(["@lead-dev".to_string()].as_slice())
        );

        assert_eq!(
            co.owners_of("src/lib.rs"),
            Some(["@rust-team".to_string()].as_slice())
        );
    }

    #[test]
    fn test_no_match() {
        let content = "*.js @js-team\n";
        let co = Codeowners::parse(content);

        assert_eq!(co.owners_of("main.rs"), None);
    }

    #[test]
    fn test_multiple_owners() {
        let content = "*.rs @alice @bob\n";
        let co = Codeowners::parse(content);

        assert_eq!(
            co.owners_of("main.rs"),
            Some(["@alice".to_string(), "@bob".to_string()].as_slice())
        );
    }
}
