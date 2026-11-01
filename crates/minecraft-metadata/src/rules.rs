use crate::models::{OsRule, Rule};

pub fn is_rule_allowed(rules: &[Rule], current_os: &str, current_arch: &str) -> bool {
    if rules.is_empty() {
        return true;
    }

    let mut allowed = false;

    for rule in rules {
        let mut matches = true;

        if let Some(os_rule) = &rule.os {
            if !matches_os(os_rule, current_os, current_arch) {
                matches = false;
            }
        }

        // Features kuralları (örn: is_demo_user, has_custom_resolution) varsayılan olarak standart başlatıcıda false
        if let Some(features) = &rule.features {
            if features.values().any(|&v| v) {
                matches = false;
            }
        }

        if matches {
            allowed = rule.action == "allow";
        }
    }

    allowed
}

fn matches_os(os_rule: &OsRule, current_os: &str, current_arch: &str) -> bool {
    if let Some(ref os_name) = os_rule.name {
        if os_name != current_os {
            return false;
        }
    }

    if let Some(ref arch) = os_rule.arch {
        if arch != current_arch && !(arch == "x86_64" && current_arch == "x64") {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rules_windows_x64() {
        let rules = vec![
            Rule {
                action: "allow".to_string(),
                os: None,
                features: None,
            },
            Rule {
                action: "disallow".to_string(),
                os: Some(OsRule {
                    name: Some("osx".to_string()),
                    arch: None,
                    version: None,
                }),
                features: None,
            },
        ];

        assert!(is_rule_allowed(&rules, "windows", "x64"));
        assert!(!is_rule_allowed(&rules, "osx", "x64"));
    }
}

