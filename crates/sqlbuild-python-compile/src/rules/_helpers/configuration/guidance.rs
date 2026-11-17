//! Rule inspection text and the generated rules skill, rendered and installed natively.

use std::path::{Path, PathBuf};

use pyo3::exceptions::PyOSError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_rules::build_identity::main::native_build_identity::native_build_identity;
use sqlbuild_rules::errors::{SkillInstallError, SkillInstallErrorKind};
use sqlbuild_rules::models::RulesConfig;
use sqlbuild_rules::rule_catalogue::main::render_skills_body::render_skills_body;
use sqlbuild_rules::rule_catalogue::main::ruleset_fingerprint::ruleset_fingerprint;
use sqlbuild_rules::rule_catalogue::main::skill_lock_name::skill_lock_name;
use sqlbuild_rules::rule_catalogue::models::{FingerprintRule, OptionText, RuleText};

use crate::rules::_helpers::configuration::catalogue::native_rules_config;
use crate::rules::_helpers::configuration::catalogue::references_custom_rules;
use crate::rules::_helpers::configuration::catalogue::rule_catalogue;
use crate::rules::_helpers::configuration::catalogue::select_rules;
use crate::rules::_helpers::configuration::catalogue::verify_custom_rules;
use crate::rules::_helpers::configuration::selection::rules_error;
use crate::rules::_helpers::configuration::skills::owned_skill;
use crate::rules::_helpers::custom::custom_rule_evidence::custom_rule_implementation_fingerprint;
use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;
use sqlbuild_python_base::records::main::fields::field::field as record_field;
use sqlbuild_python_base::records::main::fields::mapping_get_or::mapping_get_or;

pub(crate) const FILELOCK_MODULE: &str = "filelock";
pub(crate) const SKILL_PATHS: [&str; 3] = [
    ".agents/skills/sqlbuild-rules/SKILL.md",
    ".claude/skills/sqlbuild-rules/SKILL.md",
    ".opencode/skills/sqlbuild-rules/SKILL.md",
];
const SKILL_LOCK_DIRECTORY: &str = "sqlbuild-rules-skill-locks";
pub(crate) const FRESH_SKILL: &str = "fresh";

fn text_field(object: &Bound<'_, PyAny>, name: &str) -> PyResult<String> {
    record_field(object, name)?.extract()
}

fn repr_text(value: &Bound<'_, PyAny>) -> PyResult<String> {
    value.repr()?.extract()
}

/// The text view of one Python `Rule`, with effective options under `config` when given.
pub(crate) fn rule_text(
    rule: &Bound<'_, PyAny>,
    config: Option<&Bound<'_, PyAny>>,
) -> PyResult<RuleText> {
    let code: String = text_field(rule, "code")?;
    let configured: Option<Bound<'_, PyAny>> = match config {
        Some(config) => record_field(config, "rule_options")?
            .cast_into::<PyDict>()?
            .get_item(&code)?,
        None => None,
    };
    let mut options: Vec<OptionText> = Vec::new();
    for option in record_field(rule, "options")?.try_iter()? {
        let option: Bound<'_, PyAny> = option?;
        let name: String = text_field(&option, "name")?;
        let default: Bound<'_, PyAny> = record_field(&option, "default")?;
        let effective: Bound<'_, PyAny> = match &configured {
            Some(values) => mapping_get_or(values, &name, &default)?,
            None => default.clone(),
        };
        options.push(OptionText {
            name,
            description: text_field(&option, "description")?,
            default_repr: repr_text(&default)?,
            effective_repr: repr_text(&effective)?,
        });
    }
    let guidance: Bound<'_, PyAny> = record_field(rule, "guidance")?;
    Ok(RuleText {
        code,
        family: text_field(rule, "family")?,
        slug: text_field(rule, "slug")?,
        message: text_field(rule, "message")?,
        remediation: text_field(rule, "remediation")?,
        enabled_by_default: record_field(rule, "enabled_by_default")?.is_truthy()?,
        custom: record_field(rule, "custom")?.is_truthy()?,
        source: record_field(rule, "source")?.extract()?,
        options,
        good_example: if guidance.is_none() {
            None
        } else {
            Some(text_field(&guidance, "good_example")?)
        },
    })
}

fn fingerprint_rule(
    rule: &Bound<'_, PyAny>,
    project_dir: &Bound<'_, PyAny>,
) -> PyResult<FingerprintRule> {
    let custom: bool = record_field(rule, "custom")?.is_truthy()?;
    Ok(FingerprintRule {
        code: text_field(rule, "code")?,
        family: text_field(rule, "family")?,
        slug: text_field(rule, "slug")?,
        message: text_field(rule, "message")?,
        remediation: text_field(rule, "remediation")?,
        source: record_field(rule, "source")?.extract()?,
        implementation_hash: if custom {
            custom_rule_implementation_fingerprint(rule, project_dir, None)?
        } else {
            native_build_identity().to_owned()
        },
    })
}

/// The generated skill content and the ruleset fingerprint it records.
pub(crate) fn render_skills(
    config: &Bound<'_, PyAny>,
    project_dir: &Bound<'_, PyAny>,
) -> PyResult<(String, String)> {
    let py: Python<'_> = config.py();
    let native: RulesConfig = native_rules_config(config)?;
    let catalogue = rule_catalogue(config, project_dir, references_custom_rules(config)?)?;
    let rules = select_rules(&catalogue, config, project_dir, None)?;
    let mut custom_selected: bool = false;
    for rule in &rules {
        custom_selected |= record_field(rule, "custom")?.is_truthy()?;
    }
    if custom_selected {
        verify_custom_rules(&rules, project_dir)?;
    }
    let fingerprints: Vec<FingerprintRule> = rules
        .iter()
        .map(|rule| fingerprint_rule(rule, project_dir))
        .collect::<PyResult<_>>()?;
    let texts: Vec<RuleText> = rules
        .iter()
        .map(|rule| rule_text(rule, Some(config)))
        .collect::<PyResult<_>>()?;
    let (body, fingerprint): (String, Result<String, String>) = compiler_guard(|| {
        Ok((
            render_skills_body(&texts, &native),
            ruleset_fingerprint(&fingerprints, &native),
        ))
    })?;
    let fingerprint: String = fingerprint.map_err(|message| rules_error(py, &message, None))?;
    Ok((owned_skill(py, &body, &fingerprint)?, fingerprint))
}

pub(crate) fn current_content(path: &Path) -> PyResult<Option<String>> {
    if !path.is_file() {
        return Ok(None);
    }
    std::fs::read_to_string(path).map(Some).map_err(PyErr::from)
}

pub(crate) fn install_error(py: Python<'_>, error: SkillInstallError) -> PyErr {
    let raised: PyErr = match error.kind {
        SkillInstallErrorKind::Refused => rules_error(py, &error.message, None),
        SkillInstallErrorKind::Io => PyOSError::new_err(error.message),
    };
    for note in error.notes {
        let _ = raised.value(py).call_method1("add_note", (note,));
    }
    raised
}

pub(crate) fn lock_path(project_dir: &Path) -> PyResult<PathBuf> {
    let canonical: String = project_dir.canonicalize()?.display().to_string();
    let canonical: String = if cfg!(windows) {
        canonical.to_lowercase().replace('/', "\\")
    } else {
        canonical
    };
    let directory: PathBuf = std::env::temp_dir().join(SKILL_LOCK_DIRECTORY);
    std::fs::create_dir_all(&directory)?;
    Ok(directory.join(skill_lock_name(&canonical)))
}
