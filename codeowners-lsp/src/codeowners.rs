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

    /// Find all matching owners for a file path (relative to workspace root).
    /// Returns owners split into "other" (non-last matching rules) and
    /// "effective" (last matching rule, which GitHub considers the actual owner).
    pub fn all_owners_of(&self, file_path: &str) -> Option<AllOwners> {
        let path = file_path.strip_prefix('/').unwrap_or(file_path);

        let matching_rules: Vec<&Rule> = self
            .rules
            .iter()
            .filter(|rule| rule.matcher.is_match(path))
            .collect();

        if matching_rules.is_empty() {
            return None;
        }

        let (last, rest) = matching_rules.split_last().unwrap();
        let effective: Vec<String> = last.owners.clone();

        let mut other = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for owner in &effective {
            seen.insert(owner.as_str());
        }
        for rule in rest {
            for owner in &rule.owners {
                if seen.insert(owner.as_str()) {
                    other.push(owner.clone());
                }
            }
        }

        Some(AllOwners { other, effective })
    }
}

#[derive(Debug, PartialEq)]
pub struct AllOwners {
    /// Owners from all matching rules except the last, deduplicated
    pub other: Vec<String>,
    /// Owners from the last matching rule (GitHub's effective owners)
    pub effective: Vec<String>,
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
    let anchored = pattern.starts_with('/');
    let pat = pattern.strip_prefix('/').unwrap_or(pattern);

    let glob_pattern = if pat.ends_with('/') {
        // Directory pattern: match everything inside it
        format!("{pat}**")
    } else if !anchored && !pat.contains('/') {
        // No slash and not anchored: matches at any depth
        format!("**/{pat}")
    } else {
        // Anchored to root or has an internal slash: use as-is
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

        // Global fallback — only one rule matches
        assert_eq!(
            co.all_owners_of("random-file.txt"),
            Some(AllOwners {
                other: vec![],
                effective: vec!["@global-owner".into()],
            })
        );

        // Frontend override — matches * and *.js
        assert_eq!(
            co.all_owners_of("app.js"),
            Some(AllOwners {
                other: vec!["@global-owner".into()],
                effective: vec!["@frontend-team".into()],
            })
        );

        // Backend — matches * and /src/api/
        assert_eq!(
            co.all_owners_of("src/api/handler.rs"),
            Some(AllOwners {
                other: vec!["@global-owner".into()],
                effective: vec!["@backend-team".into()],
            })
        );

        // Docs — matches * and docs/*
        assert_eq!(
            co.all_owners_of("docs/readme.md"),
            Some(AllOwners {
                other: vec!["@global-owner".into()],
                effective: vec!["@docs-team".into()],
            })
        );
    }

    #[test]
    fn test_all_matching_rules() {
        let content = r#"
* @fallback
*.rs @rust-team
/src/main.rs @lead-dev
"#;
        let co = Codeowners::parse(content);

        // src/main.rs matches all three rules
        assert_eq!(
            co.all_owners_of("src/main.rs"),
            Some(AllOwners {
                other: vec!["@fallback".into(), "@rust-team".into()],
                effective: vec!["@lead-dev".into()],
            })
        );

        // src/lib.rs matches * and *.rs
        assert_eq!(
            co.all_owners_of("src/lib.rs"),
            Some(AllOwners {
                other: vec!["@fallback".into()],
                effective: vec!["@rust-team".into()],
            })
        );
    }

    #[test]
    fn test_top_level_file_pattern() {
        let content = "/README.md @team\n";
        let co = Codeowners::parse(content);

        // Should match the top-level README.md
        assert_eq!(
            co.all_owners_of("README.md"),
            Some(AllOwners {
                other: vec![],
                effective: vec!["@team".into()],
            })
        );

        // Should NOT match a nested README.md (anchored to root)
        assert_eq!(co.all_owners_of("docs/README.md"), None);
    }

    #[test]
    fn test_no_match() {
        let content = "*.js @js-team\n";
        let co = Codeowners::parse(content);

        assert_eq!(co.all_owners_of("main.rs"), None);
    }

    #[test]
    fn test_multiple_owners() {
        let content = "*.rs @alice @bob\n";
        let co = Codeowners::parse(content);

        assert_eq!(
            co.all_owners_of("main.rs"),
            Some(AllOwners {
                other: vec![],
                effective: vec!["@alice".into(), "@bob".into()],
            })
        );
    }

    #[test]
    fn test_dedup_across_rules() {
        let content = r#"
* @shared-owner @team-a
*.rs @shared-owner @team-b
"#;
        let co = Codeowners::parse(content);

        // @shared-owner appears in both rules but should only show in effective (last rule)
        assert_eq!(
            co.all_owners_of("main.rs"),
            Some(AllOwners {
                other: vec!["@team-a".into()],
                effective: vec!["@shared-owner".into(), "@team-b".into()],
            })
        );
    }
}
