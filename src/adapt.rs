//! Convert a generic subagent markdown into the target harness dialect.
//!
//! - pi / claude: passthrough (name, description, optional model + tools, body = prompt)
//! - opencode: drop `name`/`tools`, add `mode: subagent`, filename becomes `<name>.md`

use crate::model::{Harness, Subagent};

/// Quote a YAML scalar only when needed.
fn yaml_scalar(s: &str) -> String {
    let needs_quotes = s.is_empty()
        || s != s.trim()
        || s.contains([':', '#', '{', '}', '[', ']', ',', '&', '*', '!', '|', '>', '\'', '"', '%', '@', '`', '\n'])
        || s.parse::<f64>().is_ok()
        || ["true", "false", "null", "yes", "no", "~"].contains(&s.to_ascii_lowercase().as_str());
    if needs_quotes {
        format!("{:?}", s) // Rust debug formatting is a valid YAML double-quoted scalar
    } else {
        s.to_string()
    }
}

/// Render the full markdown file content for a harness.
pub fn adapt_subagent(agent: &Subagent, harness: Harness) -> String {
    let mut fm = String::new();
    match harness {
        Harness::Pi | Harness::Claude => {
            fm.push_str(&format!("name: {}\n", yaml_scalar(&agent.name)));
            fm.push_str(&format!("description: {}\n", yaml_scalar(&agent.description)));
            if let Some(model) = &agent.model {
                fm.push_str(&format!("model: {}\n", yaml_scalar(model)));
            }
            if let Some(tools) = &agent.tools {
                let list = tools
                    .iter()
                    .map(|t| format!("  - {}", yaml_scalar(t)))
                    .collect::<Vec<_>>()
                    .join("\n");
                fm.push_str(&format!("tools:\n{list}\n"));
            }
        }
        Harness::Opencode => {
            fm.push_str(&format!("description: {}\n", yaml_scalar(&agent.description)));
            fm.push_str("mode: subagent\n");
            if let Some(model) = &agent.model {
                fm.push_str(&format!("model: {}\n", yaml_scalar(model)));
            }
        }
    }
    format!("---\n{fm}---\n\n{}", agent.body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sample() -> Subagent {
        Subagent {
            name: "reviewer".into(),
            description: "Reviews code for quality: bugs, security".into(),
            model: Some("claude-sonnet-4".into()),
            tools: Some(vec!["read".into(), "grep".into(), "bash".into()]),
            body: "You are a code reviewer.".into(),
            path: PathBuf::from("/tmp/subagents/reviewer.md"),
        }
    }

    fn minimal() -> Subagent {
        Subagent {
            name: "oracle".into(),
            description: "Answers questions".into(),
            model: None,
            tools: None,
            body: "Be terse.".into(),
            path: PathBuf::from("/tmp/subagents/oracle.md"),
        }
    }

    #[test]
    fn pi_keeps_everything() {
        let out = adapt_subagent(&sample(), Harness::Pi);
        assert!(out.starts_with("---\n"));
        assert!(out.contains("name: reviewer"));
        assert!(out.contains("model: claude-sonnet-4"));
        assert!(out.contains("tools:\n  - read\n  - grep\n  - bash"));
        assert!(out.ends_with("You are a code reviewer."));
    }

    #[test]
    fn claude_passthrough() {
        let out = adapt_subagent(&sample(), Harness::Claude);
        assert!(out.contains("name: reviewer"));
        assert!(out.contains("description: \"Reviews code for quality: bugs, security\""));
    }

    #[test]
    fn opencode_drops_name_and_tools_and_adds_mode() {
        let out = adapt_subagent(&sample(), Harness::Opencode);
        assert!(!out.contains("name:"));
        assert!(!out.contains("tools:"));
        assert!(out.contains("mode: subagent"));
        assert!(out.contains("description:"));
        assert!(out.contains("model: claude-sonnet-4"));
        assert!(out.contains("You are a code reviewer."));
    }

    #[test]
    fn opencode_minimal_has_description_mode_body() {
        let out = adapt_subagent(&minimal(), Harness::Opencode);
        assert!(out.contains("description: Answers questions"));
        assert!(out.contains("mode: subagent"));
        assert!(out.contains("Be terse."));
    }

    #[test]
    fn output_is_parseable_by_scanner() {
        // Round-trip: adapted output must parse back via the scanner's frontmatter parser.
        for h in [Harness::Pi, Harness::Claude, Harness::Opencode] {
            let out = adapt_subagent(&sample(), h);
            let (fm, body) = crate::scan::split_frontmatter_for_tests(&out).unwrap();
            let parsed: serde_yaml::Value = serde_yaml::from_str(fm).unwrap();
            assert_eq!(
                parsed["description"].as_str().unwrap(),
                "Reviews code for quality: bugs, security"
            );
            assert!(body.contains("You are a code reviewer."));
        }
    }

    #[test]
    fn quoted_description_survives_yaml_parse() {
        let mut a = minimal();
        a.description = "has: colon, #hash and \"quotes\"".into();
        let out = adapt_subagent(&a, Harness::Opencode);
        let (fm, _) = crate::scan::split_frontmatter_for_tests(&out).unwrap();
        let parsed: serde_yaml::Value = serde_yaml::from_str(fm).unwrap();
        assert_eq!(
            parsed["description"].as_str().unwrap(),
            "has: colon, #hash and \"quotes\""
        );
    }
}
