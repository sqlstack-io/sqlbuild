//! One macro module's imports, calls and exported-macro dependencies.

use ruff_python_ast::ExceptHandler;
use ruff_python_ast::{ModModule, Stmt, StmtFunctionDef, StmtImportFrom};
use std::collections::{HashMap, HashSet};
use std::convert::Infallible;
use std::iter::once;

use sqlbuild_core::graph_walk::main::breadth_first_reach::breadth_first_reach;
use sqlbuild_core::graph_walk::main::find_cycle::find_cycle;
use sqlbuild_core::graph_walk::models::VisitOrder;
use sqlbuild_core::graph_walk::types::NodeSet;

use crate::cpython_tree::models::CpythonNode;
use crate::macro_modules::_helpers::declarations::module_exports;
use crate::macro_modules::_helpers::function_calls::{
    CallNames, NodeId, function_calls, runtime_nodes, unique, validate_use_locations,
};
use crate::macro_modules::_helpers::jinja_bindings::jinja_bindings;
use crate::macro_modules::_helpers::module_names::{
    import_visible, imported_module_names, is_project_macro_import, resolve_import_from,
};
use crate::macro_modules::constants::{CYCLE_SEPARATOR, STAR_IMPORT_NAME};
use crate::macro_modules::models::MacroModuleSource;

/// One parsed module of the analysed set.
pub(crate) struct ParsedModule<'a> {
    pub(crate) file_index: usize,
    pub(crate) name: String,
    pub(crate) source: &'a MacroModuleSource,
    pub(crate) module: &'a ModModule,
    pub(crate) exports: HashSet<String>,
}

/// Every parsed module of the analysed set, with the names imports resolve against.
pub(crate) struct ModuleSet<'a> {
    pub(crate) modules: Vec<ParsedModule<'a>>,
    pub(crate) positions: HashMap<String, usize>,
    pub(crate) module_names: Vec<String>,
    pub(crate) package_names: Vec<String>,
}

impl<'a> ModuleSet<'a> {
    /// The set over `modules`, in discovery order.
    pub(crate) fn new(modules: Vec<ParsedModule<'a>>, package_names: Vec<String>) -> Self {
        let positions: HashMap<String, usize> = modules
            .iter()
            .enumerate()
            .map(|(position, module)| (module.name.clone(), position))
            .collect();
        let module_names: Vec<String> = modules.iter().map(|module| module.name.clone()).collect();
        Self {
            modules,
            positions,
            module_names,
            package_names,
        }
    }

    fn is_project_import(&self, imported: &str) -> bool {
        is_project_macro_import(imported, (&self.module_names, &self.package_names))
    }
}

/// A parsed module with its exports, ready for the set.
pub(crate) fn parsed_module<'a>(
    file_index: usize,
    (name, source, module): (String, &'a MacroModuleSource, &'a ModModule),
) -> ParsedModule<'a> {
    ParsedModule {
        file_index,
        name,
        source,
        module,
        exports: module_exports(module),
    }
}

/// What the analysis of one module found.
pub(crate) struct ModuleAnalysis {
    pub(crate) file_index: usize,
    pub(crate) name: String,
    /// Each export's macro dependencies, through local helpers included.
    pub(crate) dependencies: HashMap<String, Vec<String>>,
    /// Project macro modules imported, first import first.
    pub(crate) module_dependencies: Vec<String>,
}

/// Analyse the module at `position`, or the message of the first error Python raised.
pub(crate) fn analyze_module(
    set: &ModuleSet<'_>,
    position: usize,
) -> Result<ModuleAnalysis, String> {
    let parsed: &ParsedModule<'_> = &set.modules[position];
    let display: &str = &parsed.source.relative_path.display;
    validate_no_nested_project_imports(set, parsed)?;
    let (imported, module_dependencies) = analyze_imports(set, parsed)?;
    let mut function_order: Vec<&str> = Vec::new();
    let mut functions: HashMap<&str, &StmtFunctionDef> = HashMap::new();
    for statement in &parsed.module.body {
        if let Stmt::FunctionDef(function) = statement
            && functions.insert(function.name.as_str(), function).is_none()
        {
            function_order.push(function.name.as_str());
        }
    }
    if let Some(shadowed) = function_order
        .iter()
        .filter(|name| imported.contains_key(**name))
        .min()
    {
        return Err(format!(
            "Macro module '{display}' defines function '{shadowed}' over an imported macro \
             binding; imported macros must not be replaced"
        ));
    }
    let function_names: HashSet<String> = function_order
        .iter()
        .map(|name| (*name).to_owned())
        .collect();
    let jinja: HashSet<String> = jinja_bindings(parsed.module);
    let names: CallNames<'_> = CallNames {
        imported: &imported,
        functions: &function_names,
        jinja: &jinja,
        display,
    };
    let mut direct_dependencies: HashMap<String, Vec<String>> = HashMap::new();
    let mut helper_calls: HashMap<String, Vec<String>> = HashMap::new();
    let mut runtime: HashSet<NodeId> = HashSet::new();
    for name in &function_order {
        let function: &StmtFunctionDef = functions[name];
        let nodes: Vec<CpythonNode<'_>> = runtime_nodes(function);
        runtime.extend(nodes.iter().filter_map(|node| match node {
            CpythonNode::Expr(expression) => Some(std::ptr::from_ref(*expression)),
            _ => None,
        }));
        let calls = function_calls(function, &nodes, &names)?;
        direct_dependencies.insert((*name).to_owned(), calls.dependencies);
        helper_calls.insert((*name).to_owned(), calls.helpers);
    }
    validate_local_call_cycles(&parsed.exports, &helper_calls, display)?;
    validate_use_locations(parsed.module, &names, &runtime)?;
    Ok(ModuleAnalysis {
        file_index: parsed.file_index,
        name: parsed.name.clone(),
        dependencies: export_dependencies(&parsed.exports, &direct_dependencies, &helper_calls),
        module_dependencies,
    })
}

/// Every statement inside `statement`'s compound bodies, at any depth.
fn nested_statements(statement: &Stmt) -> Vec<&Stmt> {
    let bodies: Vec<&[Stmt]> = match statement {
        Stmt::FunctionDef(node) => vec![&node.body],
        Stmt::ClassDef(node) => vec![&node.body],
        Stmt::For(node) => vec![&node.body, &node.orelse],
        Stmt::While(node) => vec![&node.body, &node.orelse],
        Stmt::If(node) => once(node.body.as_slice())
            .chain(
                node.elif_else_clauses
                    .iter()
                    .map(|clause| clause.body.as_slice()),
            )
            .collect(),
        Stmt::With(node) => vec![&node.body],
        Stmt::Match(node) => node.cases.iter().map(|case| case.body.as_slice()).collect(),
        Stmt::Try(node) => once(node.body.as_slice())
            .chain(
                node.handlers
                    .iter()
                    .map(|ExceptHandler::ExceptHandler(handler)| handler.body.as_slice()),
            )
            .chain([node.orelse.as_slice(), node.finalbody.as_slice()])
            .collect(),
        _ => Vec::new(),
    };
    bodies
        .into_iter()
        .flatten()
        .flat_map(|inner| once(inner).chain(nested_statements(inner)))
        .collect()
}

fn validate_no_nested_project_imports(
    set: &ModuleSet<'_>,
    parsed: &ParsedModule<'_>,
) -> Result<(), String> {
    let mut project_import: bool = false;
    for statement in parsed.module.body.iter().flat_map(nested_statements) {
        if !matches!(statement, Stmt::Import(_) | Stmt::ImportFrom(_)) {
            continue;
        }
        let names: Vec<String> = imported_module_names(statement, &parsed.name);
        project_import |= names.iter().any(|name| set.is_project_import(name));
    }
    if project_import {
        return Err(format!(
            "Macro module '{}' contains a nested project macro import; project macro imports \
             must be static and module-level",
            parsed.source.relative_path.display
        ));
    }
    Ok(())
}

type ImportedBindings = (HashMap<String, String>, Vec<String>);

fn analyze_imports(
    set: &ModuleSet<'_>,
    parsed: &ParsedModule<'_>,
) -> Result<ImportedBindings, String> {
    let display: &str = &parsed.source.relative_path.display;
    let mut imported: HashMap<String, String> = HashMap::new();
    let mut module_dependencies: Vec<String> = Vec::new();
    for statement in &parsed.module.body {
        if let Stmt::Import(_) = statement {
            if imported_module_names(statement, &parsed.name)
                .iter()
                .any(|name| set.is_project_import(name))
            {
                return Err(format!(
                    "Macro module '{display}' must use 'from ... import ...' for project macro \
                     imports; module imports are not supported"
                ));
            }
            continue;
        }
        let Stmt::ImportFrom(import) = statement else {
            continue;
        };
        let Some(resolved) = resolve_import_from(import, &parsed.name) else {
            return Err(format!(
                "Relative project macro import in '{}' escapes its top-level package",
                parsed.name
            ));
        };
        let Some(position) = set.positions.get(&resolved) else {
            if set.is_project_import(&resolved) {
                return Err(format!(
                    "Macro module '{display}' imports project package '{resolved}' instead of a \
                     macro module"
                ));
            }
            continue;
        };
        module_dependencies.push(resolved.clone());
        imported = record_imported_bindings(
            import,
            (parsed, &set.modules[*position]),
            &resolved,
            imported,
        )?;
    }
    Ok((imported, unique(module_dependencies)))
}

fn record_imported_bindings(
    import: &StmtImportFrom,
    (importer, imported_module): (&ParsedModule<'_>, &ParsedModule<'_>),
    resolved: &str,
    imported: HashMap<String, String>,
) -> Result<HashMap<String, String>, String> {
    let mut imported: HashMap<String, String> = imported;
    let display: &str = &importer.source.relative_path.display;
    for alias in &import.names {
        let name: &str = alias.name.as_str();
        if name == STAR_IMPORT_NAME {
            return Err(format!(
                "Macro module '{display}' must not use star imports for project macros"
            ));
        }
        if !imported_module.exports.contains(name) {
            return Err(format!(
                "Macro module '{display}' cannot import '{name}' from project macro module \
                 '{resolved}'; the name is not an exported macro"
            ));
        }
        if !import_visible(importer.source, imported_module.source) {
            return Err(format!(
                "Macro module '{display}' cannot import macro '{name}' from '{}': the imported \
                 macro is not visible from the importer scope",
                imported_module.source.relative_path.display
            ));
        }
        let binding: &str = alias.asname.as_ref().map_or(name, |asname| asname.as_str());
        if imported.contains_key(binding) {
            return Err(format!(
                "Macro module '{display}' binds project macro import '{binding}' more than once"
            ));
        }
        imported.insert(binding.to_owned(), name.to_owned());
    }
    Ok(imported)
}

fn export_dependencies(
    exports: &HashSet<String>,
    direct_dependencies: &HashMap<String, Vec<String>>,
    helper_calls: &HashMap<String, Vec<String>>,
) -> HashMap<String, Vec<String>> {
    let mut dependencies: HashMap<String, Vec<String>> = HashMap::new();
    for export in exports {
        let mut visited: VisitOrder<&str> = VisitOrder::default();
        let Ok(_) = visited.insert_new(&export.as_str());
        let Ok(()) = breadth_first_reach([export.as_str()], None, &mut visited, |function| {
            Ok::<_, Infallible>(helper_calls[*function].iter().map(String::as_str))
        });
        let mut found: Vec<String> = Vec::new();
        for function in visited.order {
            found.extend(direct_dependencies[function].iter().cloned());
            found.extend(
                helper_calls[function]
                    .iter()
                    .filter(|called| exports.contains(*called))
                    .cloned(),
            );
        }
        dependencies.insert(export.clone(), unique(found));
    }
    dependencies
}

fn validate_local_call_cycles(
    exports: &HashSet<String>,
    helper_calls: &HashMap<String, Vec<String>>,
    display: &str,
) -> Result<(), String> {
    let mut sorted: Vec<&String> = exports.iter().collect();
    sorted.sort();
    let Some(cycle) = repeated_path(sorted, helper_calls) else {
        return Ok(());
    };
    let names: Vec<&str> = cycle.iter().map(|name| name.as_str()).collect();
    Err(format!(
        "Macro call cycle in '{display}': {}",
        names.join(CYCLE_SEPARATOR)
    ))
}

/// The first dependency cycle depth first from `roots` in order, from its repeated name back to itself.
pub(crate) fn repeated_path<'a>(
    roots: Vec<&'a String>,
    edges: &'a HashMap<String, Vec<String>>,
) -> Option<Vec<&'a String>> {
    let path: Vec<&String> = find_cycle(roots, |name| edges.get(*name).into_iter().flatten())?;
    let repeated: &String = path.last()?;
    let start: usize = path.iter().position(|name| name == &repeated)?;
    Some(path[start..].to_vec())
}
