//! Rule inspection text and the generated rules skill, rendered and installed natively.

use std::path::{Path, PathBuf};

use pyo3::exceptions::PyOSError;
use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyDictMethods};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_rules::build_identity::main::native_build_identity::native_build_identity;
use sqlbuild_rules::errors::{SkillInstallError, SkillInstallErrorKind};
use sqlbuild_rules::models::RulesConfig;
use sqlbuild_rules::rule_catalogue::main::format_rule_text::format_rule_text;
use sqlbuild_rules::rule_catalogue::main::install_rendered_skills::install_rendered_skills;
use sqlbuild_rules::rule_catalogue::main::render_skills_body::render_skills_body;
use sqlbuild_rules::rule_catalogue::main::ruleset_fingerprint::ruleset_fingerprint;
use sqlbuild_rules::rule_catalogue::main::skill_lock_name::skill_lock_name;
use sqlbuild_rules::rule_catalogue::models::{FingerprintRule, OptionText, RuleText, SkillTarget};

use crate::bindings::_helpers::rules::catalogue::{
    native_rules_config, references_custom_rules, rule_catalogue, select_rules, verify_custom_rules,
};
use crate::bindings::_helpers::rules::custom_rule_evidence::custom_rule_implementation_fingerprint;
use crate::bindings::_helpers::rules::selection::rules_error;
use crate::bindings::_helpers::rules::skills::{owned_skill, skill_state};
use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::fields::field::field as record_field;
use sqlbuild_python_base::records::main::fields::mapping_get_or::mapping_get_or;

const FILELOCK_MODULE: &str = "filelock";
const SKILL_PATHS: [&str; 3] = [
    ".agents/skills/sqlbuild-rules/SKILL.md",
    ".claude/skills/sqlbuild-rules/SKILL.md",
    ".opencode/skills/sqlbuild-rules/SKILL.md",
];
const SKILL_LOCK_DIRECTORY: &str = "sqlbuild-rules-skill-locks";
const FRESH_SKILL: &str = "fresh";

fn text_field(object: &Bound<'_, PyAny>, name: &str) -> PyResult<String> {
    record_field(object, name)?.extract()
}

fn repr_text(value: &Bound<'_, PyAny>) -> PyResult<String> {
    value.repr()?.extract()
}

/// The text view of one Python `Rule`, with effective options under `config` when given.
fn rule_text(rule: &Bound<'_, PyAny>, config: Option<&Bound<'_, PyAny>>) -> PyResult<RuleText> {
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

/// `sqb rules show` text for one rule, with effective thresholds when a config is given.
#[pyfunction]
#[pyo3(signature = (*, rule, config = None))]
pub(crate) fn format_rule(
    rule: &Bound<'_, PyAny>,
    config: Option<Bound<'_, PyAny>>,
) -> PyResult<String> {
    let config: Option<Bound<'_, PyAny>> = config.filter(|config| !config.is_none());
    let native: Option<RulesConfig> = config.as_ref().map(native_rules_config).transpose()?;
    let text: RuleText = rule_text(rule, config.as_ref())?;
    compiler_guard(|| Ok(format_rule_text(&text, native.as_ref())))
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
fn render_skills(
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

fn current_content(path: &Path) -> PyResult<Option<String>> {
    if !path.is_file() {
        return Ok(None);
    }
    std::fs::read_to_string(path).map(Some).map_err(PyErr::from)
}

fn install_error(py: Python<'_>, error: SkillInstallError) -> PyErr {
    let raised: PyErr = match error.kind {
        SkillInstallErrorKind::Refused => rules_error(py, &error.message, None),
        SkillInstallErrorKind::Io => PyOSError::new_err(error.message),
    };
    for note in error.notes {
        let _ = raised.value(py).call_method1("add_note", (note,));
    }
    raised
}

fn lock_path(project_dir: &Path) -> PyResult<PathBuf> {
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

/// Install every owned rules skill target, or with `check` report whether all are fresh.
#[pyfunction]
#[pyo3(signature = (*, config, project_dir, check))]
pub(crate) fn install_rules_skills(
    config: &Bound<'_, PyAny>,
    project_dir: &Bound<'_, PyAny>,
    check: bool,
) -> PyResult<bool> {
    let py: Python<'_> = config.py();
    let (content, fingerprint): (String, String) = render_skills(config, project_dir)?;
    let root: PathBuf = project_dir.extract()?;
    let targets = || -> PyResult<Vec<SkillTarget>> {
        let mut targets: Vec<SkillTarget> = Vec::with_capacity(SKILL_PATHS.len());
        for relative in SKILL_PATHS {
            let path: PathBuf = root.join(relative);
            let state: String = skill_state(current_content(&path)?.as_deref(), &fingerprint);
            targets.push(SkillTarget { path, state });
        }
        Ok(targets)
    };
    if check {
        return Ok(targets()?.iter().all(|target| target.state == FRESH_SKILL));
    }
    let lock: Bound<'_, PyAny> =
        imported(py, FILELOCK_MODULE, "FileLock")?.call1((lock_path(&root)?,))?;
    let _ = lock.call_method0("acquire")?;
    let installed = (|| -> PyResult<()> {
        let targets: Vec<SkillTarget> = targets()?;
        compiler_guard(|| Ok(install_rendered_skills(&content, &targets)))?
            .map_err(|error| install_error(py, error))
    })();
    let released = lock.call_method0("release");
    installed?;
    let _ = released?;
    Ok(true)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(format_rule, module)?)?;
    module.add_function(wrap_pyfunction!(install_rules_skills, module)?)?;
    Ok(())
}
