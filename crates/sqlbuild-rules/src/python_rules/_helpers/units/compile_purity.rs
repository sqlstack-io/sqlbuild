//! Compile-time purity: the Python a compile runs reaches no subprocess, network, database or remote client.

use std::collections::hash_map::Entry;
use std::collections::{BTreeSet, HashMap};

use fensu_facts::ProgramHandle;
use fensu_facts::facts::models::{ImportRow, RuleNamedCallRow};
use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};
use sqlbuild_pysource::node_units::main::deferred_spans::deferred_spans;
use sqlbuild_pysource::node_units::main::node_unit_module::node_unit_module;
use sqlbuild_pysource::node_units::models::{DeferredSpan, UnitStatementKind};

use crate::models::{EvaluateRequest, Fault, PythonSource};
use crate::python_rules::_helpers::engine::faults::{catalogue_fault, character_column};
use crate::python_rules::_helpers::engine::programs::Programs;
use crate::python_rules::constants::{
    APPROVED_LOADER_BOUNDARIES_OPTION, COMPILE_TIME_PURITY_CODE, CONNECTING_FUNCTIONS,
    CONNECTING_PACKAGES, DEFERRED_CODE_MARKERS, EXTERNAL_CALL_PREFIXES, EXTERNAL_CALLS,
    FACTORY_DECORATORS, PACKAGE_INIT_MODULE,
};

/// The external call a followed call first reaches, with its file and line.
#[derive(Clone, Debug)]
struct Sink {
    call: String,
    path: String,
    line: u32,
}

/// A walk over project functions that remembers the external call each one reaches.
struct Walk<'a> {
    programs: &'a Programs<'a>,
    approved: &'a BTreeSet<String>,
    /// Per source, the code its import never runs, whose calls the walk skips.
    deferred: &'a [Vec<DeferredSpan>],
    reached: HashMap<String, Option<Sink>>,
    bodies: HashMap<usize, HashMap<String, Vec<&'a RuleNamedCallRow>>>,
}

/// Faults of compile-time purity when `codes` selects it: each root call that reaches an external system.
pub(crate) fn compile_purity_faults(
    request: &EvaluateRequest,
    codes: &BTreeSet<&str>,
) -> Result<Vec<Fault>, String> {
    if !codes.contains(COMPILE_TIME_PURITY_CODE) || request.python_sources.is_empty() {
        return Ok(Vec::new());
    }
    let approved: BTreeSet<String> = request
        .config
        .rule_option_strings(COMPILE_TIME_PURITY_CODE, APPROVED_LOADER_BOUNDARIES_OPTION)
        .into_iter()
        .collect();
    let programs = Programs::new(&request.python_sources, request.python_version);
    if let Some(module) = approved
        .iter()
        .find(|name| programs.module_index(name).is_some())
    {
        return Err(format!(
            "rules.rule_options.{COMPILE_TIME_PURITY_CODE}.{APPROVED_LOADER_BOUNDARIES_OPTION} names \
             the module {module}; list the function that loads the data instead"
        ));
    }
    (0..request.python_sources.len())
        .into_par_iter()
        .try_for_each(|index| programs.program(index).and(Ok(())))?;
    let deferred: Vec<Vec<DeferredSpan>> = request
        .python_sources
        .par_iter()
        .map(|source| source_deferred_spans(source, request.python_version))
        .collect();
    let mut walk = Walk {
        programs: &programs,
        approved: &approved,
        deferred: &deferred,
        reached: HashMap::new(),
        bodies: HashMap::new(),
    };
    let mut faults: Vec<Fault> = Vec::new();
    for (index, source) in request.python_sources.iter().enumerate() {
        let program: &ProgramHandle = programs.program(index)?;
        for call in root_calls(
            (program, &deferred[index]),
            source,
            (&approved, request.python_version),
        ) {
            if let Some(sink) = walk.reach(index, call)? {
                faults.extend(purity_fault(source, call, &sink));
            }
        }
    }
    faults.sort_by(|left, right| {
        (&left.path, left.line, left.column).cmp(&(&right.path, right.line, right.column))
    });
    faults.dedup_by(|left, right| {
        left.path == right.path && left.line == right.line && left.column == right.column
    });
    Ok(faults)
}

impl<'a> Walk<'a> {
    /// The external call `call` in source `index` reaches, directly or through project functions.
    fn reach(&mut self, index: usize, call: &RuleNamedCallRow) -> Result<Option<Sink>, String> {
        let programs: &'a Programs<'a> = self.programs;
        let source: &PythonSource = &programs.sources[index];
        let module: Vec<String> = module_parts(&source.relative_path);
        let parts: Vec<String> = absolute_parts(
            call,
            &programs.program(index)?.reference_rows().imports,
            &package_parts(&source.relative_path, &module),
        );
        if let Some(external) = external_call(&parts) {
            return Ok(Some(Sink {
                call: external,
                path: source.relative_path.clone(),
                line: call.line,
            }));
        }
        let (target, function): (String, String) = match parts.as_slice() {
            [] => return Ok(None),
            [function] => (module.join("."), function.clone()),
            [owner @ .., function] => (owner.join("."), function.clone()),
        };
        let Some(target_index) = programs.module_index(&target) else {
            return Ok(None);
        };
        let identity: String = format!("{target}.{function}");
        if self.approved.contains(&identity) {
            return Ok(None);
        }
        if let Some(known) = self.reached.get(&identity) {
            return Ok(known.clone());
        }
        self.reached.insert(identity.clone(), None);
        let mut sink: Option<Sink> = None;
        for body_call in self.body(target_index, &function)? {
            sink = self.reach(target_index, body_call)?;
            if sink.is_some() {
                break;
            }
        }
        self.reached.insert(identity, sink.clone());
        Ok(sink)
    }

    /// The calls written directly in top-level function `function` of source `index`.
    fn body(&mut self, index: usize, function: &str) -> Result<Vec<&'a RuleNamedCallRow>, String> {
        let programs: &'a Programs<'a> = self.programs;
        if let Entry::Vacant(entry) = self.bodies.entry(index) {
            let mut bodies: HashMap<String, Vec<&'a RuleNamedCallRow>> = HashMap::new();
            for call in programs.program(index)?.named_call_rows() {
                if let ([owner], []) = (
                    call.enclosing_functions.as_slice(),
                    call.enclosing_classes.as_slice(),
                ) && !deferred(&self.deferred[index], call)
                {
                    bodies.entry(owner.name.clone()).or_default().push(call);
                }
            }
            entry.insert(bodies);
        }
        Ok(self
            .bodies
            .get(&index)
            .and_then(|bodies| bodies.get(function))
            .cloned()
            .unwrap_or_default())
    }
}

/// The lambda bodies and `__main__` block of `source`, which its import never runs.
fn source_deferred_spans(source: &PythonSource, python_version: (u8, u8)) -> Vec<DeferredSpan> {
    if !DEFERRED_CODE_MARKERS
        .iter()
        .any(|marker| source.text.contains(marker))
    {
        return Vec::new();
    }
    deferred_spans(&source.text, python_version).unwrap_or_default()
}

/// Whether `call` sits in code its module's import never runs.
fn deferred(spans: &[DeferredSpan], call: &RuleNamedCallRow) -> bool {
    spans
        .iter()
        .any(|span| span.contains(call.line as usize, call.column as usize))
}

/// The calls `source`'s import runs: module and class scope and unapproved `@factory` bodies, outside `spans`.
fn root_calls<'p>(
    (program, spans): (&'p ProgramHandle, &[DeferredSpan]),
    source: &PythonSource,
    (approved, python_version): (&BTreeSet<String>, (u8, u8)),
) -> Vec<&'p RuleNamedCallRow> {
    let module: String = module_parts(&source.relative_path).join(".");
    let factories: BTreeSet<String> = factory_names(source, python_version)
        .into_iter()
        .filter(|name| !approved.contains(&format!("{module}.{name}")))
        .collect();
    let mut calls: Vec<&RuleNamedCallRow> = Vec::new();
    for call in program.named_call_rows() {
        let root: bool = match call.enclosing_functions.as_slice() {
            [] => true,
            [owner] => call.enclosing_classes.is_empty() && factories.contains(&owner.name),
            _ => false,
        };
        if root && !deferred(spans, call) {
            calls.push(call);
        }
    }
    calls
}

/// The top-level functions of `source` decorated as factories.
fn factory_names(source: &PythonSource, python_version: (u8, u8)) -> BTreeSet<String> {
    let mut names: BTreeSet<String> = BTreeSet::new();
    if !FACTORY_DECORATORS
        .iter()
        .any(|decorator| source.text.contains(decorator))
    {
        return names;
    }
    let Some(module) = node_unit_module(&source.text, python_version) else {
        return names;
    };
    for statement in module.statements {
        if let UnitStatementKind::Function(function) = statement.kind
            && function
                .decorators
                .iter()
                .any(|decorator| FACTORY_DECORATORS.contains(&decorator.name.as_str()))
        {
            names.insert(function.name);
        }
    }
    names
}

/// The dotted module of a project file, such as `python.orders.export` for `python/orders/export/__init__.py`.
fn module_parts(relative_path: &str) -> Vec<String> {
    let mut parts: Vec<String> = relative_path
        .trim_end_matches(".py")
        .split('/')
        .map(str::to_owned)
        .collect();
    if parts.last().is_some_and(|last| last == PACKAGE_INIT_MODULE) {
        parts.pop();
    }
    parts
}

/// The package a relative import in the file at `relative_path` starts from.
fn package_parts(relative_path: &str, module: &[String]) -> Vec<String> {
    if relative_path.ends_with(&format!("{PACKAGE_INIT_MODULE}.py")) {
        module.to_vec()
    } else {
        module[..module.len().saturating_sub(1)].to_vec()
    }
}

/// The dotted name `call` targets once its first name is resolved through `imports`; empty when unresolvable.
fn absolute_parts(
    call: &RuleNamedCallRow,
    imports: &[ImportRow],
    package: &[String],
) -> Vec<String> {
    let parts: &[String] = &call.reference.parts;
    let Some(first) = parts.first() else {
        return Vec::new();
    };
    for row in imports {
        let Some(alias) = row.aliases.iter().find(|alias| &alias.bound_name == first) else {
            continue;
        };
        let imported: Vec<String> = alias.imported_name.split('.').map(str::to_owned).collect();
        if !row.from_import && imported.first() == Some(first) {
            return parts.to_vec();
        }
        let mut resolved: Vec<String> = if row.from_import {
            let Some(base) = import_base(row, package) else {
                return Vec::new();
            };
            base
        } else {
            Vec::new()
        };
        resolved.extend(imported);
        resolved.extend(parts.iter().skip(1).cloned());
        return resolved;
    }
    parts.to_vec()
}

/// The module a `from ... import` reads from, resolving leading dots against `package`.
fn import_base(row: &ImportRow, package: &[String]) -> Option<Vec<String>> {
    let mut base: Vec<String> = if row.relative_level == 0 {
        Vec::new()
    } else {
        package.to_vec()
    };
    for _ in 1..row.relative_level {
        base.pop()?;
    }
    base.extend(row.module_parts.iter().cloned());
    Some(base)
}

/// The dotted call when `parts` names a call that reaches an external system.
fn external_call(parts: &[String]) -> Option<String> {
    let joined: String = parts.join(".");
    let connecting: bool = parts.len() > 1
        && CONNECTING_PACKAGES.contains(&parts[0].as_str())
        && parts
            .last()
            .is_some_and(|last| CONNECTING_FUNCTIONS.contains(&last.as_str()));
    (connecting
        || EXTERNAL_CALLS.contains(&joined.as_str())
        || EXTERNAL_CALL_PREFIXES
            .iter()
            .any(|prefix| joined.starts_with(prefix)))
    .then_some(joined)
}

/// The fault on root `call` in `source`, naming the external call it reaches and where.
fn purity_fault(source: &PythonSource, call: &RuleNamedCallRow, sink: &Sink) -> Option<Fault> {
    let detail: String = if sink.path == source.relative_path && sink.line == call.line {
        format!("calls {}", sink.call)
    } else {
        format!("reaches {} at {}:{}", sink.call, sink.path, sink.line)
    };
    catalogue_fault(
        COMPILE_TIME_PURITY_CODE,
        &source.relative_path,
        (
            u64::from(call.line.max(1)),
            character_column(&source.text, call.line, call.column),
        ),
        &detail,
    )
}
