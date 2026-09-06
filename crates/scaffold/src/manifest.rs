//! `template.toml` — the per-template manifest that declares the custom
//! variables a template needs beyond the built-in
//! `project_name` / `crate_name` / `author` / `sdk_version` / `edition` set.
//!
//! A manifest is optional. When a template ships one, `soroban-forge new`
//! resolves each declared variable from (in order) `--var name=value`, the
//! manifest default, an interactive prompt, and finally errors out if the
//! variable is required and still unset.
//!
//! ```toml
//! # templates/my-token/template.toml
//! description = "a token with a configurable symbol"
//!
//! [[variables]]
//! name = "token_symbol"
//! prompt = "Token symbol"
//! default = "TKN"
//!
//! [[variables]]
//! name = "admin_address"
//! prompt = "Admin account (G...)"
//! required = true
//! ```

use serde::{Deserialize, Serialize};
use soroban_forge_core::render::Vars;
use soroban_forge_core::{ForgeError, Result};

/// The file name a template uses to declare its metadata and variables.
/// It configures generation and is never copied into the generated project.
pub const MANIFEST_FILE: &str = "template.toml";

/// Variables the scaffolder always derives itself. A template may *use* them,
/// but it must not redeclare them and `--var` must not override them.
pub const RESERVED_VARS: &[&str] = &[
    "project_name",
    "crate_name",
    "author",
    "sdk_version",
    "edition",
];

/// One custom variable declared by a template.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateVariable {
    /// Placeholder name, used as `{{name}}` inside the template.
    pub name: String,
    /// Human-readable prompt shown interactively. Falls back to `name`.
    #[serde(default)]
    pub prompt: Option<String>,
    /// Value used when nothing is supplied — also the interactive default.
    #[serde(default)]
    pub default: Option<String>,
    /// When true (and no default exists), generation fails rather than
    /// rendering an empty value. Defaults to true: a template that bothers to
    /// declare a variable normally needs it.
    #[serde(default = "default_true")]
    pub required: bool,
}

fn default_true() -> bool {
    true
}

impl TemplateVariable {
    /// The text shown when asking the user for this variable.
    pub fn prompt_text(&self) -> &str {
        self.prompt.as_deref().unwrap_or(&self.name)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
struct PostGenerate {
    #[serde(default)]
    hints: Vec<String>,
}

/// A parsed `template.toml`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct TemplateManifest {
    /// Optional one-line description (overrides the built-in catalogue entry).
    #[serde(default)]
    pub description: Option<String>,
    /// Custom variables this template needs.
    #[serde(default, rename = "variable", alias = "variables")]
    pub variables: Vec<TemplateVariable>,
    #[serde(default)]
    post_generate: PostGenerate,
}

impl TemplateManifest {
    /// Parse a `template.toml`'s contents. Empty/missing manifests are
    /// represented by the caller as `TemplateManifest::default()`, not by
    /// calling this with empty input.
    pub fn parse(raw: &str, template_name: &str) -> Result<Self> {
        let manifest: Self = toml::from_str(raw).map_err(|e| ForgeError::Config {
            path: format!("templates/{template_name}/template.toml").into(),
            message: e.to_string(),
        })?;
        validate_manifest(&manifest)?;
        Ok(manifest)
    }

    /// Post-generation hints to print after `next steps`, in declared order.
    pub fn hints(&self) -> &[String] {
        &self.post_generate.hints
    }
}

/// Parse a `template.toml`, rejecting manifests that redeclare a reserved
/// variable or the same name twice.
pub fn parse_manifest(raw: &str) -> Result<TemplateManifest> {
    let manifest: TemplateManifest = toml::from_str(raw)
        .map_err(|e| ForgeError::Template(format!("invalid {MANIFEST_FILE}: {e}")))?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

fn validate_manifest(manifest: &TemplateManifest) -> Result<()> {
    let mut seen: Vec<&str> = Vec::new();
    for var in &manifest.variables {
        if var.name.trim().is_empty() {
            return Err(ForgeError::Template(format!(
                "{MANIFEST_FILE} declares a variable with an empty name"
            )));
        }
        if RESERVED_VARS.contains(&var.name.as_str()) {
            return Err(ForgeError::Template(format!(
                "{MANIFEST_FILE} redeclares `{}`, which soroban-forge derives itself",
                var.name
            )));
        }
        if seen.contains(&var.name.as_str()) {
            return Err(ForgeError::Template(format!(
                "{MANIFEST_FILE} declares `{}` twice",
                var.name
            )));
        }
        seen.push(&var.name);
    }
    Ok(())
}

/// Parse one `--var name=value` pair.
pub fn parse_var_assignment(raw: &str) -> Result<(String, String)> {
    let (key, value) = raw.split_once('=').ok_or_else(|| {
        ForgeError::InvalidArgument(format!(
            "`{raw}` is not a valid --var assignment (expected NAME=VALUE)"
        ))
    })?;
    let key = key.trim();
    if key.is_empty() {
        return Err(ForgeError::InvalidArgument(format!(
            "`{raw}` is not a valid --var assignment (the name is empty)"
        )));
    }
    if RESERVED_VARS.contains(&key) {
        return Err(ForgeError::InvalidArgument(format!(
            "`{key}` is derived by soroban-forge and cannot be set with --var"
        )));
    }
    Ok((key.to_string(), value.to_string()))
}

/// Collect every `--var NAME=VALUE` into a map, failing on the first bad pair.
pub fn parse_var_assignments<'a>(raw: impl IntoIterator<Item = &'a str>) -> Result<Vars> {
    let mut vars = Vars::new();
    for item in raw {
        let (key, value) = parse_var_assignment(item)?;
        vars.insert(key, value);
    }
    Ok(vars)
}

/// How a missing variable should be obtained. Injected so the resolution logic
/// stays unit-testable without a terminal.
pub trait VarPrompter {
    /// Ask for `var`, returning the entered value. `None` means "no answer
    /// available" (not a TTY, EOF, or the user just pressed enter with no
    /// default), which makes resolution fall through to the default/error.
    fn ask(&mut self, var: &TemplateVariable) -> Option<String>;
}

/// A prompter that never asks — the non-interactive path.
pub struct NoPrompt;

impl VarPrompter for NoPrompt {
    fn ask(&mut self, _var: &TemplateVariable) -> Option<String> {
        None
    }
}

/// Resolve every variable a template declares.
///
/// Precedence per variable: `--var` → interactive answer → manifest default.
/// A required variable with no value left is a hard error naming the flag that
/// would have supplied it, so non-interactive runs fail fast.
pub fn resolve_variables(
    manifest: &TemplateManifest,
    supplied: &Vars,
    prompter: &mut dyn VarPrompter,
) -> Result<Vars> {
    let mut resolved = Vars::new();

    for var in &manifest.variables {
        if let Some(value) = supplied.get(&var.name) {
            resolved.insert(var.name.clone(), value.clone());
            continue;
        }

        let answer = prompter.ask(var).filter(|a| !a.is_empty());
        let value = answer.or_else(|| var.default.clone());

        match value {
            Some(value) => {
                resolved.insert(var.name.clone(), value);
            }
            None if var.required => {
                return Err(ForgeError::InvalidArgument(format!(
                    "template variable `{}` is required but was not supplied \
                     (pass `--var {}=VALUE`, or run without --yes in a terminal to be prompted)",
                    var.name, var.name
                )));
            }
            None => {
                resolved.insert(var.name.clone(), String::new());
            }
        }
    }

    // Variables passed with --var that the manifest does not declare are still
    // rendered: templates may use placeholders the manifest doesn't list.
    for (key, value) in supplied {
        resolved.entry(key.clone()).or_insert_with(|| value.clone());
    }

    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest_with(vars: &[TemplateVariable]) -> TemplateManifest {
        TemplateManifest {
            description: None,
            variables: vars.to_vec(),
            post_generate: PostGenerate::default(),
        }
    }

    fn var(name: &str, default: Option<&str>, required: bool) -> TemplateVariable {
        TemplateVariable {
            name: name.into(),
            prompt: None,
            default: default.map(String::from),
            required,
        }
    }

    /// Answers a fixed queue of values, recording what it was asked for.
    struct ScriptedPrompter {
        answers: Vec<Option<String>>,
        asked: Vec<String>,
    }

    impl VarPrompter for ScriptedPrompter {
        fn ask(&mut self, v: &TemplateVariable) -> Option<String> {
            self.asked.push(v.name.clone());
            if self.answers.is_empty() {
                None
            } else {
                self.answers.remove(0)
            }
        }
    }

    #[test]
    fn parses_a_manifest_with_variables() {
        let manifest = parse_manifest(
            r#"
description = "demo"

[[variables]]
name = "token_symbol"
prompt = "Token symbol"
default = "TKN"
"#,
        )
        .unwrap();
        assert_eq!(manifest.description.as_deref(), Some("demo"));
        assert_eq!(manifest.variables.len(), 1);
        let v = &manifest.variables[0];
        assert_eq!(v.name, "token_symbol");
        assert_eq!(v.prompt_text(), "Token symbol");
        assert_eq!(v.default.as_deref(), Some("TKN"));
        assert!(v.required, "variables are required unless told otherwise");
    }

    #[test]
    fn empty_manifest_declares_no_variables() {
        assert_eq!(parse_manifest("").unwrap(), TemplateManifest::default());
    }

    #[test]
    fn manifest_may_not_redeclare_reserved_variables() {
        let err = parse_manifest("[[variables]]\nname = \"crate_name\"\n").unwrap_err();
        assert!(err.to_string().contains("crate_name"), "{err}");
    }

    #[test]
    fn manifest_may_not_declare_a_variable_twice() {
        let err = parse_manifest("[[variables]]\nname = \"a\"\n\n[[variables]]\nname = \"a\"\n")
            .unwrap_err();
        assert!(err.to_string().contains("twice"), "{err}");
    }

    #[test]
    fn invalid_toml_is_a_template_error() {
        let err = parse_manifest("this is not toml").unwrap_err();
        assert!(err.to_string().starts_with("template error"), "{err}");
    }

    #[test]
    fn parses_var_assignments() {
        let vars = parse_var_assignments(["symbol=TKN", "supply=100"]).unwrap();
        assert_eq!(vars["symbol"], "TKN");
        assert_eq!(vars["supply"], "100");
    }

    #[test]
    fn var_value_may_contain_equals_signs() {
        let (k, v) = parse_var_assignment("motto=a=b").unwrap();
        assert_eq!((k.as_str(), v.as_str()), ("motto", "a=b"));
    }

    #[test]
    fn var_value_may_be_empty() {
        let (k, v) = parse_var_assignment("note=").unwrap();
        assert_eq!((k.as_str(), v.as_str()), ("note", ""));
    }

    #[test]
    fn rejects_var_without_equals() {
        let err = parse_var_assignment("symbol").unwrap_err();
        assert!(err.to_string().contains("NAME=VALUE"), "{err}");
    }

    #[test]
    fn rejects_var_overriding_a_reserved_name() {
        let err = parse_var_assignment("crate_name=oops").unwrap_err();
        assert!(
            err.to_string().contains("cannot be set with --var"),
            "{err}"
        );
    }
    #[test]
    fn missing_manifest_is_default() {
        let m = TemplateManifest::default();
        assert_eq!(m.description, None);
        assert!(m.variables.is_empty());
        assert!(m.hints().is_empty());
    }

    #[test]
    fn parses_description_and_variables() {
        let raw = r#"
description = "SEP-41 fungible token"

[[variable]]
name = "token_symbol"
prompt = "Token symbol"
default = "MYT"

[[variable]]
name = "token_decimals"
prompt = "Decimals"
default = "7"

[post_generate]
hints = ["deploy with --decimals 7", "run cargo test first"]
"#;
        let m = TemplateManifest::parse(raw, "token").unwrap();
        assert_eq!(m.description.as_deref(), Some("SEP-41 fungible token"));
        assert_eq!(m.variables.len(), 2);
        assert_eq!(m.variables[0].name, "token_symbol");
        assert_eq!(m.variables[0].default.as_deref(), Some("MYT"));
        assert_eq!(
            m.hints(),
            &[
                "deploy with --decimals 7".to_string(),
                "run cargo test first".to_string()
            ]
        );
    }

    #[test]
    fn supplied_values_win_over_defaults_and_prompts() {
        let manifest = manifest_with(&[var("symbol", Some("TKN"), true)]);
        let supplied = Vars::from([("symbol".to_string(), "USDC".to_string())]);
        let mut prompter = ScriptedPrompter {
            answers: vec![Some("NOPE".into())],
            asked: Vec::new(),
        };
        let resolved = resolve_variables(&manifest, &supplied, &mut prompter).unwrap();
        assert_eq!(resolved["symbol"], "USDC");
        assert!(prompter.asked.is_empty(), "supplied vars must not prompt");
    }

    #[test]
    fn missing_variables_are_prompted() {
        let manifest = manifest_with(&[var("symbol", None, true)]);
        let mut prompter = ScriptedPrompter {
            answers: vec![Some("USDC".into())],
            asked: Vec::new(),
        };
        let resolved = resolve_variables(&manifest, &Vars::new(), &mut prompter).unwrap();
        assert_eq!(resolved["symbol"], "USDC");
        assert_eq!(prompter.asked, vec!["symbol"]);
    }

    #[test]
    fn an_empty_answer_falls_back_to_the_default() {
        let manifest = manifest_with(&[var("symbol", Some("TKN"), true)]);
        let mut prompter = ScriptedPrompter {
            answers: vec![Some(String::new())],
            asked: Vec::new(),
        };
        let resolved = resolve_variables(&manifest, &Vars::new(), &mut prompter).unwrap();
        assert_eq!(resolved["symbol"], "TKN");
    }

    #[test]
    fn non_interactive_runs_fall_back_to_defaults() {
        let manifest = manifest_with(&[var("symbol", Some("TKN"), true)]);
        let resolved = resolve_variables(&manifest, &Vars::new(), &mut NoPrompt).unwrap();
        assert_eq!(resolved["symbol"], "TKN");
    }

    #[test]
    fn non_interactive_runs_fail_fast_on_a_required_variable() {
        let manifest = manifest_with(&[var("symbol", None, true)]);
        let err = resolve_variables(&manifest, &Vars::new(), &mut NoPrompt).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("`symbol` is required"), "{msg}");
        assert!(msg.contains("--var symbol=VALUE"), "{msg}");
    }

    #[test]
    fn optional_variables_render_as_empty_when_unset() {
        let manifest = manifest_with(&[var("note", None, false)]);
        let resolved = resolve_variables(&manifest, &Vars::new(), &mut NoPrompt).unwrap();
        assert_eq!(resolved["note"], "");
    }

    #[test]
    fn undeclared_supplied_vars_are_still_rendered() {
        let resolved = resolve_variables(
            &TemplateManifest::default(),
            &Vars::from([("extra".to_string(), "x".to_string())]),
            &mut NoPrompt,
        )
        .unwrap();
        assert_eq!(resolved["extra"], "x");
    }

    #[test]
    fn variables_and_hints_default_to_empty() {
        let m = TemplateManifest::parse(r#"description = "x""#, "x").unwrap();
        assert!(m.variables.is_empty());
        assert!(m.hints().is_empty());
    }

    #[test]
    fn invalid_toml_is_a_config_error() {
        let err = TemplateManifest::parse("not [valid", "token").unwrap_err();
        assert!(matches!(err, ForgeError::Config { .. }));
    }
}
