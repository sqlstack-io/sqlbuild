//! The project's Python sources, each parsed at most once and only when a rule needs it.

use std::collections::HashMap;
use std::sync::OnceLock;

use fensu_facts::ProgramHandle;

use crate::models::PythonSource;

/// Parsed programs for a request's Python sources, filled on first use.
#[derive(Debug)]
pub(crate) struct Programs<'a> {
    pub sources: &'a [PythonSource],
    python_version: (u8, u8),
    parsed: Vec<OnceLock<Result<ProgramHandle, String>>>,
    by_path: HashMap<&'a str, usize>,
}

impl<'a> Programs<'a> {
    pub(crate) fn new(sources: &'a [PythonSource], python_version: (u8, u8)) -> Self {
        Self {
            sources,
            python_version,
            parsed: sources.iter().map(|_| OnceLock::new()).collect(),
            by_path: sources
                .iter()
                .enumerate()
                .map(|(index, source)| (source.relative_path.as_str(), index))
                .collect(),
        }
    }

    /// The parsed program of source `index`; a file the parser rejects fails the run.
    pub(crate) fn program(&self, index: usize) -> Result<&ProgramHandle, String> {
        self.parsed[index]
            .get_or_init(|| {
                let source: &PythonSource = &self.sources[index];
                ProgramHandle::parse_source(
                    &source.text,
                    self.python_version.0,
                    self.python_version.1,
                )
                .map_err(|failure| {
                    format!(
                        "{}:{}:{}: cannot parse Python source: {}",
                        source.relative_path,
                        failure.line.max(1),
                        failure.column + 1,
                        failure.message
                    )
                })
            })
            .as_ref()
            .map_err(Clone::clone)
    }

    /// The index of the source at project-relative `path`.
    pub(crate) fn index_of(&self, path: &str) -> Option<usize> {
        self.by_path.get(path).copied()
    }

    /// The source of dotted `module` inside the runtime package, as a module file or a package initializer.
    pub(crate) fn module_index(&self, module: &str) -> Option<usize> {
        let path: String = module.replace('.', "/");
        self.index_of(&format!("{path}.py"))
            .or_else(|| self.index_of(&format!("{path}/__init__.py")))
    }
}
