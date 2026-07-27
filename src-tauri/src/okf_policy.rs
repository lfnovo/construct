use glob::{MatchOptions, Pattern};
use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

pub(crate) const IGNORE_FILE_NAME: &str = ".constructignore";
const MAX_IGNORE_FILE_BYTES: u64 = 64 * 1024;
const MAX_IGNORE_RULES: usize = 1_024;

#[derive(Debug)]
struct IgnoreRule {
    pattern: Pattern,
    negated: bool,
    basename_only: bool,
    directory_only: bool,
}

#[derive(Debug, Default)]
pub(crate) struct ConformancePolicy {
    rules: Vec<IgnoreRule>,
}

fn normalize_path(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    normalized
        .strip_prefix("./")
        .unwrap_or(&normalized)
        .to_string()
}

fn compile_rule(value: &str, source: &str) -> Result<Option<IgnoreRule>, String> {
    let value = value.trim();
    if value.is_empty() || value.starts_with('#') {
        return Ok(None);
    }

    let (negated, value) = value
        .strip_prefix('!')
        .map_or((false, value), |value| (true, value.trim()));
    if value.is_empty() {
        return Err(format!("{source} contains an empty negated pattern"));
    }

    let anchored = value.starts_with('/');
    let directory_only = value.ends_with('/');
    let mut pattern = value
        .trim_start_matches('/')
        .trim_end_matches('/')
        .replace('\\', "/");
    if pattern.is_empty() {
        return Err(format!("{source} contains an empty pattern"));
    }

    let basename_only = !anchored
        && (!pattern.contains('/')
            || pattern
                .strip_prefix("**/")
                .is_some_and(|value| !value.contains('/')));
    if basename_only {
        pattern = pattern.strip_prefix("**/").unwrap_or(&pattern).to_string();
    }

    let pattern = Pattern::new(&pattern)
        .map_err(|error| format!("invalid ignore pattern '{value}' in {source}: {error}"))?;
    Ok(Some(IgnoreRule {
        pattern,
        negated,
        basename_only,
        directory_only,
    }))
}

impl IgnoreRule {
    fn matches(&self, candidate: &str) -> bool {
        let options = MatchOptions {
            case_sensitive: true,
            require_literal_separator: true,
            require_literal_leading_dot: false,
        };
        let candidate = normalize_path(candidate);
        if self.basename_only {
            let components = candidate.split('/').collect::<Vec<_>>();
            let candidates = if self.directory_only && components.len() > 1 {
                &components[..components.len() - 1]
            } else {
                &components[components.len().saturating_sub(1)..]
            };
            return candidates
                .iter()
                .any(|component| self.pattern.matches_with(component, options));
        }
        if self.directory_only {
            let mut prefix = PathBuf::new();
            for component in Path::new(&candidate)
                .components()
                .take(Path::new(&candidate).components().count().saturating_sub(1))
            {
                prefix.push(component);
                let prefix = prefix.to_string_lossy().replace('\\', "/");
                if self.pattern.matches_with(&prefix, options) {
                    return true;
                }
            }
            return false;
        }
        self.pattern.matches_with(&candidate, options)
    }
}

impl ConformancePolicy {
    pub(crate) fn load(
        root: &Path,
        command_patterns: &[String],
        use_ignore_file: bool,
    ) -> Result<Self, String> {
        let mut sourced_patterns = Vec::new();
        if use_ignore_file {
            let path = root.join(IGNORE_FILE_NAME);
            match fs::symlink_metadata(&path) {
                Ok(metadata) => {
                    if metadata.file_type().is_symlink() {
                        return Err(format!(
                            "'{}' must be a regular file inside the bundle root",
                            path.display()
                        ));
                    }
                    if metadata.len() > MAX_IGNORE_FILE_BYTES {
                        return Err(format!(
                            "{} exceeds the {} KB safety limit",
                            path.display(),
                            MAX_IGNORE_FILE_BYTES / 1024
                        ));
                    }
                    let content = fs::read_to_string(&path)
                        .map_err(|error| format!("could not read '{}': {error}", path.display()))?;
                    sourced_patterns.extend(content.lines().enumerate().map(|(index, value)| {
                        (
                            value.to_string(),
                            format!("{}:{}", path.display(), index + 1),
                        )
                    }));
                }
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(format!("could not inspect '{}': {error}", path.display()));
                }
            }
        }
        sourced_patterns.extend(
            command_patterns
                .iter()
                .cloned()
                .map(|value| (value, "--exclude".to_string())),
        );

        let mut rules = Vec::new();
        for (value, source) in sourced_patterns {
            if let Some(rule) = compile_rule(&value, &source)? {
                rules.push(rule);
                if rules.len() > MAX_IGNORE_RULES {
                    return Err(format!(
                        "OKF conformance policy cannot exceed {MAX_IGNORE_RULES} patterns"
                    ));
                }
            }
        }
        Ok(Self { rules })
    }

    pub(crate) fn is_ignored(&self, relative_path: &str) -> bool {
        let mut ignored = false;
        for rule in &self.rules {
            if rule.matches(relative_path) {
                ignored = !rule.negated;
            }
        }
        ignored
    }

    pub(crate) fn ignored_paths<'a>(
        &self,
        relative_paths: impl IntoIterator<Item = &'a str>,
    ) -> Vec<String> {
        let mut paths = relative_paths
            .into_iter()
            .filter(|path| self.is_ignored(path))
            .map(str::to_string)
            .collect::<Vec<_>>();
        paths.sort_by(|left, right| {
            left.to_ascii_lowercase()
                .cmp(&right.to_ascii_lowercase())
                .then_with(|| left.cmp(right))
        });
        paths
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_directory(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "construct-okf-policy-{}-{name}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create temporary directory");
        path
    }

    #[test]
    fn basename_patterns_apply_at_every_depth() {
        let root = temporary_directory("basename");
        fs::write(
            root.join(IGNORE_FILE_NAME),
            "# Agent instructions are not OKF concepts.\nAGENTS.md\n**/SKILL.md\n",
        )
        .expect("write ignore file");
        let policy = ConformancePolicy::load(&root, &[], true).expect("load policy");
        assert!(policy.is_ignored("AGENTS.md"));
        assert!(policy.is_ignored("nested/AGENTS.md"));
        assert!(policy.is_ignored("skills/example/SKILL.md"));
        assert!(!policy.is_ignored("concept.md"));
        fs::remove_dir_all(root).expect("remove temporary directory");
    }

    #[test]
    fn directory_rules_and_negation_use_last_match() {
        let root = temporary_directory("negation");
        fs::write(
            root.join(IGNORE_FILE_NAME),
            "drafts/\n!drafts/published.md\n",
        )
        .expect("write ignore file");
        let policy = ConformancePolicy::load(&root, &[], true).expect("load policy");
        assert!(policy.is_ignored("drafts/private.md"));
        assert!(!policy.is_ignored("drafts/published.md"));
        fs::remove_dir_all(root).expect("remove temporary directory");
    }

    #[test]
    fn command_patterns_compose_with_the_repository_file() {
        let root = temporary_directory("compose");
        fs::write(root.join(IGNORE_FILE_NAME), "AGENTS.md\n").expect("write ignore file");
        let policy = ConformancePolicy::load(&root, &["generated/**".to_string()], true)
            .expect("load policy");
        assert!(policy.is_ignored("AGENTS.md"));
        assert!(policy.is_ignored("generated/result.md"));
        fs::remove_dir_all(root).expect("remove temporary directory");
    }

    #[test]
    fn the_rule_limit_does_not_count_comments_or_blank_lines() {
        let root = temporary_directory("comments");
        let mut policy_file = "# comment\n\n".repeat(MAX_IGNORE_RULES + 1);
        policy_file.push_str("AGENTS.md\n");
        fs::write(root.join(IGNORE_FILE_NAME), policy_file).expect("write ignore file");
        let policy = ConformancePolicy::load(&root, &[], true).expect("load policy");
        assert!(policy.is_ignored("AGENTS.md"));
        fs::remove_dir_all(root).expect("remove temporary directory");
    }
}
