//! How each Python rule maps to Fensu and which files it reads.

/// The Python files a rule applies to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PythonFiles {
    /// Only the runtime package under `python/`, whose layout the rule checks.
    Package,
    /// Every supported project Python file, including hooks, UDFs and orchestration modules.
    All,
    /// Only the project's Python tests under `tests/python/` and `tests/providers/`.
    Tests,
}

/// One `SQBRPYTHON` rule and the Fensu core rule it runs, if any.
#[derive(Clone, Copy, Debug)]
pub struct PythonRuleSpec {
    pub code: &'static str,
    /// The Fensu code it evaluates; `None` for SQLBuild's own layout rules.
    pub fensu_code: Option<&'static str>,
    pub slug: &'static str,
    /// Fensu's family, which with `owner` decides the file a project-wide finding is reported on.
    pub family: &'static str,
    pub owner: &'static str,
    pub applies: PythonFiles,
    pub enabled_by_default: bool,
    /// Whole-number options as name, default and description, named as Fensu names its thresholds.
    pub options: &'static [(&'static str, u32, &'static str)],
    pub message: &'static str,
    pub remediation: &'static str,
}

/// One project Python file placed the way Fensu places a file under its scan root.
#[derive(Clone, Debug)]
pub(crate) struct PythonTarget {
    /// The file's index in the request's Python sources.
    pub source: usize,
    pub repository_path: String,
    /// Whether the file belongs to the runtime package under `python/`.
    pub package: bool,
    /// The directory the file is placed under: `python` for package files, else its parent.
    pub root: String,
    /// Path parts below `root`.
    pub relative_parts: Vec<String>,
    pub role: Option<String>,
    pub is_main_module: bool,
    pub is_entry_module: bool,
    /// Fensu's scope for the file: `test` for project Python tests, else the root scope.
    pub scope: &'static str,
}

/// One `evaluate_core_rules` call on a file: the Fensu codes it runs and the context they read.
#[derive(Debug)]
pub(crate) struct RuleCall {
    pub codes: Vec<String>,
    pub context: fensu_native::NativeRuleContext,
    pub queries: Vec<fensu_native::NativeProjectQuery>,
}

/// What every call of one run reads.
#[derive(Debug)]
pub(crate) struct CallInputs<'a> {
    pub request: &'a crate::models::EvaluateRequest,
    pub settings: crate::python_rules::_helpers::engine::contexts::ContextSettings,
    pub programs: &'a crate::python_rules::_helpers::engine::programs::Programs<'a>,
    /// Cached findings by file from the last run with the same ruleset.
    pub cached: &'a std::collections::BTreeMap<String, PythonCacheEntry>,
    /// The digest of the ruleset and interpreter version the cached findings must match.
    pub fingerprint: &'a str,
    /// The digest of every Python source, for rules that read the whole project.
    pub project_digest: String,
}

/// One file's cached Python findings and the project answers they were computed from.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct PythonCacheEntry {
    /// The digest of the file, its rules and thresholds, and the ruleset.
    pub identity: String,
    /// The project questions the file's rules asked, as kind, path and argument.
    pub queries: Vec<(String, String, String)>,
    /// The digest of their answers; a changed answer makes the entry stale.
    pub answers: String,
    pub faults: Vec<crate::models::Fault>,
}

/// The Python rules' findings, with the cache entries of every file that ran Fensu's rules.
#[derive(Debug, Default)]
pub(crate) struct PythonEvaluation {
    pub faults: Vec<crate::models::Fault>,
    pub entries: std::collections::BTreeMap<String, PythonCacheEntry>,
    pub hits: usize,
    pub misses: usize,
}

/// One file's planned calls before it is parsed: Fensu codes and thresholds per call.
#[derive(Debug)]
pub(crate) struct FilePlan<'a> {
    pub target: &'a PythonTarget,
    pub calls: Vec<(Vec<String>, std::collections::HashMap<String, u32>)>,
    pub identity: String,
}

/// Planned files split into cached entries that still hold and files to evaluate again.
#[derive(Debug, Default)]
pub(crate) struct CacheSplit<'a> {
    pub fresh: Vec<(String, PythonCacheEntry)>,
    pub stale: Vec<FilePlan<'a>>,
}

/// One parsed project file, its path parts from the project root, and its text.
#[derive(Debug)]
pub(crate) struct ParsedFile<'a> {
    pub path: &'a str,
    pub parts: Vec<&'a str>,
    pub text: &'a str,
    pub module: sqlbuild_pysource::node_units::models::UnitModule,
}

/// One parsed module under `python/` and the unit folder it sits in.
#[derive(Debug)]
pub(crate) struct UnitFile<'a> {
    pub path: &'a str,
    /// Path parts below `python/`.
    pub parts: Vec<&'a str>,
    pub module: sqlbuild_pysource::node_units::models::UnitModule,
    /// The nearest unit folder below `python/` that contains the file, at any depth.
    pub unit: Option<String>,
}

/// The project's parsed `python/` modules, its unit folders and every module name it holds.
#[derive(Debug)]
pub(crate) struct UnitProject<'a> {
    pub files: Vec<UnitFile<'a>>,
    /// Folders below `python/` that hold a `node.py`, below a domain.
    pub units: std::collections::BTreeSet<String>,
    /// Every project Python path.
    pub paths: std::collections::BTreeSet<&'a str>,
    /// Each file's position in `files`, by path.
    pub index: std::collections::BTreeMap<&'a str, usize>,
    /// The dotted module of every unit's `node.py`, such as `python.orders.daily.node`.
    pub node_modules: std::collections::BTreeSet<String>,
    /// Every project module and package as a dotted name, such as `python.orders.constants`.
    pub modules: std::collections::BTreeSet<String>,
}
