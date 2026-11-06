//! `build_test_inputs`: SQL test blocks expanded, classified and checked natively.

use std::collections::{HashMap, HashSet};
use std::convert::Infallible;

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use sqlbuild_analysis::compiled_project::main::text_position::text_position;
use sqlbuild_analysis::compiled_project::models::TextPosition;
use sqlbuild_analysis::compiler::main::sql_test_classification::extract_sql_tests;
use sqlbuild_analysis::compiler::models::{
    SqlTestClassification, SqlTestCteText, SqlTestExtractionFailure, SqlTestRequest,
};
use sqlbuild_analysis::sql_test_attachment::main::authored_test_offset::authored_test_offset;
use sqlbuild_analysis::sql_test_attachment::main::sql_test_cte_location::sql_test_cte_location;
use sqlbuild_analysis::sql_test_attachment::main::sql_test_reads::sql_test_reads;
use sqlbuild_analysis::sql_test_attachment::main::stored_sql_tests::stored_sql_tests;
use sqlbuild_analysis::sql_test_attachment::main::test_without_target_model::test_without_target_model;
use sqlbuild_analysis::sql_test_attachment::main::unresolvable_test_references::unresolvable_test_references;
use sqlbuild_analysis::sql_test_attachment::models::{
    CteLocation, ModelTestCtes, PythonTextMatcher, SqlTestCheckFailure, SqlTestHelperProblem,
    SqlTestReadFacts,
};
use sqlbuild_attachments::ceremonial_select::main::omitted_select_offset::omitted_select_offset;
use sqlbuild_attachments::ceremonial_select::models::OmittedSelect;
use sqlbuild_attachments::cursor_intrinsics::main::intrinsic_free::intrinsic_free;
use sqlbuild_attachments::cursor_intrinsics::models::IntrinsicCheck;
use sqlbuild_attachments::functions::main::table_function_arity_error::table_function_arity_error;
use sqlbuild_attachments::test_parameters::main::parameter_references::parameter_references;
use sqlbuild_attachments::test_targets::main::unknown_test_target::unknown_test_target;
use sqlbuild_attachments::test_targets::models::{TargetCatalog, TestTargets};
use sqlbuild_render::macro_calls::main::macro_call_names::macro_call_names;
use sqlbuild_sqltext::compiler::models::DeclarationReferenceStop;
use sqlbuild_sqltext::sql_references::main::extract_sql_references::extract_sql_references;
use sqlbuild_sqltext::sql_references::models::{ReferenceExtraction, SqlReference};
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

use crate::bindings::_helpers::cache::sql_test_scan_store::SqlTestScanStore;
use crate::bindings::_helpers::compile_context::delta::record_test_source_reads;
use crate::bindings::_helpers::compiled_project::attachment_inputs::{
    SqlTestSettings, TestBlockRow, TestCaseRow, TestFileRow,
};
use crate::bindings::_helpers::compiled_views::source_views::replaced;
use crate::bindings::_helpers::input_loops::model_loop::{
    LoopContext, NativeModelLoop, compile_error, helped_compile_error,
};
use crate::bindings::_helpers::input_loops::model_loop_calls::{
    CallOrigin, origin_references, report_invalid_call,
};
use crate::bindings::_helpers::input_loops::source_loop::{
    AuthoredExpansion, expanded_authored_sql, scoped_declarations,
};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::render::sql_macro_tests::without_with_list_calls;

const MODEL_MODE: &str = "model";
const MACRO_MODE: &str = "macro";
const TABLE_FUNCTION_MODE: &str = "table_fn";
const REF_KIND: &str = "ref";
const SOURCE_KIND: &str = "source";
const UDF_KIND: &str = "udf";
const TABLE_FUNCTION_KIND: &str = "table_fn";
const DECLARATION_CALLS: [&str; 2] = ["enum", "const"];
const OMITTED_CEREMONIAL_SELECT: &str = "\nSELECT 1";
const HELPER_REFERENCE_CODE: &str = "P013";
const SQL_TEST_RESOURCE: &str = "sql_test";
const MAX_SQL_VALUE_SIZE: usize = 1_000_000;
const ENUM_VISIBILITY: &str = "enum_visibility";
const CONSTANT_VISIBILITY: &str = "constant_visibility";
const MACRO_RECORDS: &str = "macro_records";
const MACRO_VISIBILITY: &str = "macro_visibility";

/// The loop, its context, the settings and the project names every test resolves against.
pub(crate) struct TestScope<'a, 'py> {
    pub(crate) model_loop: &'a NativeModelLoop,
    pub(crate) context: &'a LoopContext<'a, 'py>,
    pub(crate) settings: &'a SqlTestSettings,
    pub(crate) catalog: TargetCatalog,
    pub(crate) functions: HashSet<String>,
    pub(crate) table_function_arities: HashMap<String, usize>,
}

/// One test case expanded before extraction.
struct ExpandedTest<'a, 'py> {
    file: &'a TestFileRow,
    block: &'a TestBlockRow,
    case: Option<&'a TestCaseRow>,
    sql: String,
    tested: Vec<String>,
    usages: Bound<'py, PyTuple>,
}

/// `(file path, relative path, contents)` of an authored test or scenario file.
pub(crate) type FileOrigin<'a> = (&'a str, &'a str, &'a str);

impl<'py> TestScope<'_, 'py> {
    fn py(&self) -> Python<'py> {
        self.context.host.py()
    }

    fn error(&self, message: &str) -> PyErr {
        compile_error(self.context.classes, message, None)
    }

    /// Every test case expanded, the batch classified, then each test checked and recorded.
    pub(crate) fn attached(&self, files: &[TestFileRow]) -> PyResult<Vec<Py<PyAny>>> {
        let mut expanded: Vec<ExpandedTest<'_, 'py>> = Vec::new();
        for file in files {
            for block in &file.blocks {
                if block.cases.is_empty() {
                    expanded.push(self.expanded(file, block, None)?);
                }
                for case in &block.cases {
                    expanded.push(self.expanded(file, block, Some(case))?);
                }
            }
        }
        let classified: Vec<SqlTestClassification> = self.classified(&expanded)?;
        let mut inputs: Vec<Py<PyAny>> = Vec::new();
        for (test, classification) in expanded.into_iter().zip(classified) {
            if let Some(input) = self.checked(test, classification)? {
                inputs.push(input);
            }
        }
        Ok(inputs)
    }

    /// One case's SQL with parameters, variables, declarations and macros expanded.
    fn expanded<'a>(
        &self,
        file: &'a TestFileRow,
        block: &'a TestBlockRow,
        case: Option<&'a TestCaseRow>,
    ) -> PyResult<ExpandedTest<'a, 'py>> {
        let py: Python<'py> = self.py();
        let TestFileRow {
            file_path,
            relative_path,
            stem,
            contents,
            ..
        } = file;
        let TestBlockRow {
            name,
            sql_body,
            mode,
            parameters,
            ..
        } = block;
        let test_name: &str = name.as_deref().unwrap_or(stem);
        let resource: Bound<'py, PyAny> = self
            .context
            .classes
            .visibility
            .resource_identity
            .call1((self.settings.test_kind.bind(py), test_name))?;
        let declarations: Bound<'py, PyAny> =
            scoped_declarations(self.context, file_path, &resource)?;
        let parameter_sql: String = match case {
            Some(case) => {
                self.with_parameters(file_path, (test_name, parameters), sql_body, case)?
            }
            None => sql_body.clone(),
        };
        let origin: FileOrigin<'_> = (file_path, relative_path, contents);
        let (classified_sql, with_list_macros) = without_with_list_calls(&parameter_sql);
        if let Some(called) = with_list_macros.first().filter(|_| mode != MACRO_MODE) {
            return Err(self.error(&format!(
                "SQL test '{test_name}' in {relative_path} calls CTE-list SQL macro \
                 @sql.{called}() in its WITH list; only a mode macro test may call one there"
            )));
        }
        let tested: Vec<String> = if mode == MODEL_MODE {
            Vec::new()
        } else {
            let direct: SqlTestClassification =
                self.raw_direct(&classified_sql, (file, block), mode)?;
            self.tested_names(&direct, mode, origin, &with_list_macros)?
        };
        let macro_mode: bool = mode == MACRO_MODE;
        let expansion_declarations: Bound<'py, PyAny> = if macro_mode {
            let changes: Bound<'py, PyDict> = PyDict::new(py);
            changes.set_item("consumer", py.None())?;
            replaced(py, &declarations, &changes)?.into_bound(py)
        } else {
            declarations.clone()
        };
        let expansion: AuthoredExpansion<'py> = expanded_authored_sql(
            self.context,
            (&parameter_sql, file_path),
            &expansion_declarations,
        )?;
        let sql: String = checked_body(
            self.model_loop,
            self.context,
            &expansion.sql,
            &format!("SQL test '{test_name}'"),
        )?;
        let usages: Bound<'py, PyTuple> = if macro_mode {
            self.macro_usages(&parameter_sql, &resource, &declarations)?
        } else {
            expansion.usages
        };
        Ok(ExpandedTest {
            file,
            block,
            case,
            sql,
            tested,
            usages,
        })
    }

    /// `expand_test_parameters`, then `SQL test ... declares unused parameters`.
    fn with_parameters(
        &self,
        file_path: &str,
        test: (&str, &[String]),
        sql: &str,
        case: &TestCaseRow,
    ) -> PyResult<String> {
        let (test_name, parameters) = test;
        let (_, case_name, values) = case;
        let declared: Vec<String> = values.iter().map(|(name, _)| name.clone()).collect();
        let owner: String = format!("SQL test '{test_name}' case '{case_name}' in '{file_path}'");
        let scan = parameter_references(sql, &declared, &owner);
        let characters: Vec<char> = sql.chars().collect();
        let mut expanded: String = String::with_capacity(sql.len());
        let mut cursor: usize = 0;
        let mut used: HashSet<&str> = HashSet::new();
        for reference in &scan.references {
            expanded.extend(&characters[cursor..reference.start]);
            let value: &Py<PyAny> = values
                .iter()
                .find(|(name, _)| *name == reference.name)
                .map(|(_, value)| value)
                .ok_or_else(|| self.error(&reference.name))?;
            expanded.push_str(&self.rendered_parameter(
                (test_name, case_name),
                &reference.name,
                value,
            )?);
            used.insert(&reference.name);
            cursor = reference.end;
        }
        expanded.extend(&characters[cursor..]);
        if let Some(message) = scan.error {
            return Err(self.error(&message));
        }
        let unused: Vec<&str> = parameters
            .iter()
            .map(String::as_str)
            .filter(|name| !used.contains(name))
            .collect();
        if !unused.is_empty() {
            return Err(self.error(&format!(
                "SQL test '{test_name}' declares unused parameters: {} in case '{case_name}'",
                unused.join(", ")
            )));
        }
        Ok(expanded)
    }

    /// `_render_parameter`: the adapter's rendering of one value, within the size limit.
    fn rendered_parameter(
        &self,
        test: (&str, &str),
        name: &str,
        value: &Py<PyAny>,
    ) -> PyResult<String> {
        let (test_name, case_name) = test;
        let label: String = format!("SQL test '{test_name}' case '{case_name}' parameter '{name}'");
        let (text, error): (Option<String>, Option<String>) = self
            .settings
            .render_parameter
            .bind(self.py())
            .call1((value,))?
            .extract()?;
        let problem: Option<String> = match (&text, error) {
            (_, Some(error)) => Some(error),
            (Some(text), None) if text.len() > MAX_SQL_VALUE_SIZE => Some(format!(
                "{label} rendered value is {} bytes; maximum is {MAX_SQL_VALUE_SIZE} bytes",
                text.len()
            )),
            _ => None,
        };
        if let Some(problem) = problem {
            return Err(self.error(&format!(
                "{label} could not be rendered by adapter '{}': {problem}",
                self.settings.adapter_name
            )));
        }
        Ok(text.unwrap_or_default())
    }

    /// `_validate_raw_direct_logic_test_ctes`: the direct test before expansion, its P012s reported.
    fn raw_direct(
        &self,
        sql: &str,
        test: (&TestFileRow, &TestBlockRow),
        mode: &str,
    ) -> PyResult<SqlTestClassification> {
        let (file, block) = test;
        let request: SqlTestRequest = SqlTestRequest {
            sql: sql.to_owned(),
            file_label: file.relative_path.clone(),
            mode: mode.to_owned(),
            raw: true,
            authored: false,
            scenario: false,
        };
        let classification: SqlTestClassification =
            match extract_sql_tests(&[request], &self.model_loop.syntax) {
                Ok(mut classified) => classified.remove(0),
                Err(failure) => return Err(self.located(&failure, file, (block, sql))),
            };
        if let SqlTestClassification::Direct {
            helpers,
            actual,
            expected,
            invalid_calls: true,
            ..
        } = &classification
        {
            let origin: FileOrigin<'_> = (&file.file_path, &file.relative_path, &file.contents);
            if self.authored_invalid_calls(file, block, &actual.0)? == 0 {
                for cte in helpers.iter().chain([expected]) {
                    origin_references(self.context, &self.model_loop.syntax, &cte.1, origin)?;
                }
            }
        }
        Ok(classification)
    }

    /// `report_authored_invalid_calls`: P012 for each call outside the actual CTE, located.
    fn authored_invalid_calls(
        &self,
        file: &TestFileRow,
        block: &TestBlockRow,
        actual: &str,
    ) -> PyResult<usize> {
        let syntax = &self.model_loop.syntax;
        let request: SqlTestRequest = SqlTestRequest {
            sql: block.sql_body.clone(),
            file_label: file.relative_path.clone(),
            mode: MODEL_MODE.to_owned(),
            raw: false,
            authored: true,
            scenario: false,
        };
        let ctes: Vec<(String, usize, String)> = match extract_sql_tests(&[request], syntax) {
            Ok(classified) => match classified.into_iter().next() {
                Some(SqlTestClassification::Authored { ctes }) => ctes,
                _ => Vec::new(),
            },
            Err(failure) => {
                return Err(helped_compile_error(
                    self.context.classes,
                    &failure.message,
                    failure.help.as_deref(),
                ));
            }
        };
        let mut reported: usize = 0;
        for (_, body_start, body) in ctes.iter().filter(|cte| cte.0 != actual) {
            let scan = match extract_sql_references(body, syntax) {
                ReferenceExtraction::Extracted(scan) => scan,
                ReferenceExtraction::Failed(failure) => return Err(self.error(&failure.message)),
            };
            for invalid in &scan.invalid_calls {
                let Some(offset) = authored_test_offset(
                    &file.contents,
                    &block.sql_body,
                    block.sql_body_span,
                    body_start + invalid.start,
                ) else {
                    return Ok(reported);
                };
                report_invalid_call(
                    self.context,
                    invalid,
                    Some(offset),
                    CallOrigin {
                        name: None,
                        relative_path: &file.relative_path,
                        file_path: &file.file_path,
                        contents: &file.contents,
                    },
                )?;
                reported += 1;
            }
        }
        Ok(reported)
    }

    /// `_infer_tested_direct_logic_resource_names` for a macro, UDF or table function test.
    fn tested_names(
        &self,
        classification: &SqlTestClassification,
        mode: &str,
        origin: FileOrigin<'_>,
        with_list_macros: &[String],
    ) -> PyResult<Vec<String>> {
        let relative_path: &str = origin.1;
        let SqlTestClassification::Direct { actual, .. } = classification else {
            return Err(self.error(&format!(
                "SQL test file {relative_path} mode '{mode}' must define exactly one actual CTE \
                 and exactly one expected CTE"
            )));
        };
        if mode == MACRO_MODE {
            if let Some(unknown) = with_list_macros
                .iter()
                .find(|name| !self.catalog.macros.contains(*name))
            {
                return Err(self.error(&format!(
                    "SQL test file {relative_path} references unknown macro '@sql.{unknown}'"
                )));
            }
            let mut names: Vec<String> = with_list_macros.to_vec();
            for name in top_level_macro_names(&actual.1, &self.model_loop.syntax)
                .map_err(|message| self.error(message))?
            {
                if !names.contains(&name) {
                    names.push(name);
                }
            }
            if names.is_empty() {
                return Err(self.error(&format!(
                    "SQL test file {relative_path} mode 'macro' must call at least one macro in \
                     __macro_actual__"
                )));
            }
            if let Some(unknown) = names
                .iter()
                .find(|name| !self.catalog.macros.contains(*name))
            {
                return Err(self.error(&format!(
                    "SQL test file {relative_path} references unknown macro '@py.{unknown}'"
                )));
            }
            return Ok(names);
        }
        let references: Vec<SqlReference> =
            origin_references(self.context, &self.model_loop.syntax, &actual.1, origin)?;
        let table_function: bool = mode == TABLE_FUNCTION_MODE;
        if table_function {
            let calls: Vec<(&str, &str, Option<usize>)> = references
                .iter()
                .map(|reference| {
                    (
                        reference.kind,
                        reference.name.as_str(),
                        reference.call_argument_count,
                    )
                })
                .collect();
            if let Some(message) = table_function_arity_error(
                &format!("SQL test file {relative_path}"),
                &calls,
                &self.table_function_arities,
            ) {
                return Err(self.error(&message));
            }
        }
        let kind: &str = if table_function {
            TABLE_FUNCTION_KIND
        } else {
            UDF_KIND
        };
        let names: Vec<String> = unique(
            references
                .iter()
                .filter(|reference| reference.kind == kind)
                .map(|reference| reference.name.clone()),
        );
        let (label, actual_cte) = if table_function {
            ("table function", "__table_fn_actual__")
        } else {
            ("scalar UDF", "__udf_actual__")
        };
        if names.is_empty() {
            return Err(self.error(&format!(
                "SQL test file {relative_path} mode '{mode}' must call at least one {label} in \
                 {actual_cte}"
            )));
        }
        for name in &names {
            if !self.functions.contains(name) {
                return Err(self.error(&format!(
                    "SQL test file {relative_path} references unknown SQL function '{name}'"
                )));
            }
            let is_table: bool = self.catalog.table_functions.contains(name);
            if !table_function && is_table {
                return Err(self.error(&format!(
                    "SQL test file {relative_path} references table function '{name}' with \
                     __udf(); use __table_fn() for table functions"
                )));
            }
            if table_function && !is_table {
                return Err(self.error(&format!(
                    "SQL test file {relative_path} references scalar SQL function '{name}' with \
                     __table_fn(); use __udf() for scalar UDFs"
                )));
            }
        }
        Ok(names)
    }

    /// `_macro_test_declaration_usages`: declaration, then macro, usages of a macro test.
    fn macro_usages(
        &self,
        sql: &str,
        resource: &Bound<'py, PyAny>,
        declarations: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyTuple>> {
        let py: Python<'py> = self.py();
        let unique: Bound<'py, PyDict> = PyDict::new(py);
        let scans =
            sqlbuild_sqltext::compiler::main::declaration_references::scan_declaration_references(
                &[sql.to_owned()],
                true,
                Some(&self.model_loop.syntax),
            );
        if let Some(stop) = scans.iter().find_map(|scan| scan.stop) {
            let problem: &str = match stop {
                DeclarationReferenceStop::UnclosedBlockComment => "unclosed block comment",
                _ => "unclosed quoted string",
            };
            return Err(self.error(&format!(
                "Enum and constant expansion contains an {problem}"
            )));
        }
        let enum_visibility: Bound<'py, PyDict> =
            native_field(declarations, ENUM_VISIBILITY)?.downcast_into()?;
        let constant_visibility: Bound<'py, PyDict> =
            native_field(declarations, CONSTANT_VISIBILITY)?.downcast_into()?;
        for reference in scans.iter().flat_map(|scan| scan.references.iter()) {
            let visibility: &Bound<'py, PyDict> = match reference.kind {
                sqlbuild_sqltext::compiler::models::DeclarationReferenceKind::Enum => {
                    &enum_visibility
                }
                sqlbuild_sqltext::compiler::models::DeclarationReferenceKind::Constant => {
                    &constant_visibility
                }
            };
            let Some(records) = visibility.get_item(&reference.name)? else {
                continue;
            };
            for record in test_visibility(&records)? {
                unique.set_item(
                    self.usage(
                        resource,
                        &native_field(&record, "declaration")?,
                        &native_field(&record, "through")?,
                    )?,
                    py.None(),
                )?;
            }
        }
        let macro_records: Bound<'py, PyDict> =
            native_field(declarations, MACRO_RECORDS)?.downcast_into()?;
        let macro_visibility: Bound<'py, PyDict> =
            native_field(declarations, MACRO_VISIBILITY)?.downcast_into()?;
        for name in top_level_macro_names(sql, &self.model_loop.syntax)
            .map_err(|message| self.error(message))?
        {
            let Some(record) = macro_records.get_item(&name)? else {
                continue;
            };
            let identity: Bound<'py, PyAny> = native_field(&record, "identity")?;
            let visible: Vec<Bound<'py, PyAny>> = match macro_visibility.get_item(&name)? {
                Some(visibility) => test_visibility(&visibility)?,
                None => Vec::new(),
            };
            if visible.is_empty() {
                unique.set_item(
                    self.usage(resource, &identity, &py.None().into_bound(py))?,
                    py.None(),
                )?;
            }
            for record in visible {
                unique.set_item(
                    self.usage(resource, &identity, &native_field(&record, "through")?)?,
                    py.None(),
                )?;
            }
        }
        PyTuple::new(py, unique.keys())
    }

    /// One runtime `UsageRecord` of `declaration` by `consumer`.
    fn usage(
        &self,
        consumer: &Bound<'py, PyAny>,
        declaration: &Bound<'py, PyAny>,
        through: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let fields: Bound<'py, PyDict> = PyDict::new(self.py());
        fields.set_item("consumer", consumer)?;
        fields.set_item("declaration", declaration)?;
        fields.set_item("kind", self.settings.runtime_usage.bind(self.py()))?;
        fields.set_item("through", through)?;
        self.context.classes.usage_record.call((), Some(&fields))
    }

    /// `extract_expanded_sql_tests`: stored whole-file results, then the rest scanned and stored.
    fn classified(&self, tests: &[ExpandedTest<'_, 'py>]) -> PyResult<Vec<SqlTestClassification>> {
        let py: Python<'py> = self.py();
        let mut files: Vec<(&str, Vec<usize>)> = Vec::new();
        let mut positions: HashMap<&str, usize> = HashMap::new();
        for (index, test) in tests.iter().enumerate() {
            match positions.get(test.file.relative_path.as_str()) {
                Some(position) => files[*position].1.push(index),
                None => {
                    positions.insert(&test.file.relative_path, files.len());
                    files.push((&test.file.relative_path, vec![index]));
                }
            }
        }
        let store: Option<Bound<'py, SqlTestScanStore>> = self
            .settings
            .scan_store
            .as_ref()
            .map(|store| store.bind(py).clone());
        let mut results: Vec<Option<SqlTestClassification>> = tests.iter().map(|_| None).collect();
        let mut missing: Vec<(Vec<String>, &Vec<usize>)> = Vec::new();
        for (label, indexes) in &files {
            let mut parts: Vec<String> = vec![
                self.settings.scan_key.0.clone(),
                self.settings.scan_key.1.clone(),
                (*label).to_owned(),
            ];
            let mut modes: Vec<&str> = Vec::with_capacity(indexes.len());
            for index in indexes {
                parts.push(tests[*index].sql.clone());
                parts.push(tests[*index].block.mode.clone());
                modes.push(&tests[*index].block.mode);
            }
            let stored: Option<Vec<SqlTestClassification>> = match &store {
                Some(store) => store
                    .get()
                    .decoded(parts.clone(), |value| stored_sql_tests(value, &modes))?,
                None => None,
            };
            match stored {
                Some(stored) => {
                    for (index, classification) in indexes.iter().zip(stored) {
                        results[*index] = Some(classification);
                    }
                }
                None => missing.push((parts, indexes)),
            }
        }
        let mut pending: Vec<usize> = Vec::new();
        for (_, indexes) in &missing {
            pending.extend(indexes.iter().copied());
        }
        pending.sort_unstable();
        let requests: Vec<SqlTestRequest> = pending
            .iter()
            .map(|index| SqlTestRequest {
                sql: tests[*index].sql.clone(),
                file_label: tests[*index].file.relative_path.clone(),
                mode: tests[*index].block.mode.clone(),
                raw: false,
                authored: false,
                scenario: false,
            })
            .collect();
        let syntax: &LexicalSyntax = &self.model_loop.syntax;
        let classified: Vec<SqlTestClassification> = py
            .detach(|| extract_sql_tests(&requests, syntax))
            .map_err(|failure| {
                let test: &ExpandedTest<'_, 'py> = &tests[pending[failure.index]];
                self.located(&failure, test.file, (test.block, &test.sql))
            })?;
        let mut by_index: HashMap<usize, SqlTestClassification> =
            pending.into_iter().zip(classified).collect();
        for (parts, indexes) in missing {
            let mut items: Vec<SqlTestClassification> = Vec::with_capacity(indexes.len());
            for index in indexes {
                items.push(
                    by_index
                        .remove(index)
                        .ok_or_else(|| self.error("SQL test was not classified"))?,
                );
            }
            if let Some(store) = &store {
                let value: Vec<u8> =
                    serde_json::to_vec(&items).map_err(|error| self.error(&error.to_string()))?;
                store.get().stored(parts, value)?;
            }
            for (index, item) in indexes.iter().zip(items) {
                results[*index] = Some(item);
            }
        }
        results
            .into_iter()
            .map(|result| result.ok_or_else(|| self.error("SQL test was not classified")))
            .collect()
    }

    /// `located_extraction_error`: the error at its authored file line and column when exact.
    fn located(
        &self,
        failure: &SqlTestExtractionFailure,
        file: &TestFileRow,
        test: (&TestBlockRow, &str),
    ) -> PyErr {
        let (block, sql) = test;
        let offset: Option<usize> = failure
            .token
            .as_ref()
            .filter(|_| sql == block.sql_body)
            .and_then(|(_, offset)| {
                authored_test_offset(
                    &file.contents,
                    &block.sql_body,
                    block.sql_body_span,
                    *offset,
                )
            });
        located_error(
            self.context,
            failure,
            (&file.relative_path, &file.contents),
            offset,
        )
    }

    /// Check one classified test; `None` when a P013 problem was reported instead.
    fn checked(
        &self,
        test: ExpandedTest<'_, 'py>,
        classification: SqlTestClassification,
    ) -> PyResult<Option<Py<PyAny>>> {
        let py: Python<'py> = self.py();
        let TestFileRow {
            file_path,
            relative_path,
            stem,
            file: test_file,
            contents,
            ..
        } = test.file;
        let TestBlockRow {
            block: test_block,
            name,
            mode,
            ..
        } = test.block;
        let test_name: &str = name.as_deref().unwrap_or(stem);
        let origin: FileOrigin<'_> = (file_path, relative_path, contents);
        let payload: Bound<'py, PyAny> = match &classification {
            SqlTestClassification::Model {
                authored,
                macro_mocks,
                mock_models,
                mock_sources,
                mock_seeds,
                mock_dbt_refs,
                mock_table_functions,
                expected,
                expected_models,
                assertions,
                assertion_names,
                ..
            } => {
                let targets: Vec<String> =
                    assertion_targets(self.context, self.model_loop, assertions, origin)?
                        .into_iter()
                        .filter(|name| !mock_models.contains(name))
                        .collect();
                record_test_source_reads(relative_path, mock_sources.iter().map(String::as_str));
                if let Some(message) = unknown_test_target(
                    relative_path,
                    &self.catalog,
                    &TestTargets {
                        mock_models: mock_models.clone(),
                        mock_sources: mock_sources.clone(),
                        mock_seeds: mock_seeds.clone(),
                        mock_table_functions: mock_table_functions.clone(),
                        macro_mocks: macro_mocks.iter().map(|(name, _)| name.clone()).collect(),
                        expected_models: expected_models.clone(),
                        assertion_targets: targets.clone(),
                    },
                ) {
                    return Err(self.error(&message));
                }
                let ctes: ModelTestCtes<'_> = ModelTestCtes {
                    authored,
                    expected,
                    assertions,
                    mock_models,
                    mock_sources,
                    mock_seeds,
                    mock_dbt_refs,
                    mock_table_functions,
                };
                let patterns: PythonTextMatcher = PythonTextMatcher;
                let problems: Vec<SqlTestHelperProblem> =
                    self.checked_helpers(unresolvable_test_references(
                        &ctes,
                        &self.catalog.models,
                        &self.model_loop.syntax,
                        &patterns,
                    ))?;
                if !problems.is_empty() {
                    for problem in &problems {
                        self.report_helper_problem(test.file, test.block, test_name, problem)?;
                    }
                    return Ok(None);
                }
                let reads: SqlTestReadFacts = self.checked_helpers(sql_test_reads(
                    &ctes,
                    &self.model_loop.syntax,
                    &patterns,
                ))?;
                if mode == MODEL_MODE
                    && expected_models.is_empty()
                    && targets.is_empty()
                    && reads.reference_target_model_names.is_empty()
                {
                    let problem: SqlTestHelperProblem =
                        self.checked_helpers(test_without_target_model(
                            &ctes,
                            test_name,
                            &self.model_loop.syntax,
                            &patterns,
                        ))?;
                    self.report_helper_problem(test.file, test.block, test_name, &problem)?;
                    return Ok(None);
                }
                let fields: Bound<'py, PyDict> = PyDict::new(py);
                fields.set_item("authored_ctes", self.ctes(authored)?)?;
                let mocks: Bound<'py, PyDict> = PyDict::new(py);
                for (name, sql) in macro_mocks {
                    mocks.set_item(name, sql)?;
                }
                fields.set_item("macro_mocks", mocks)?;
                fields.set_item("mock_model_names", PyTuple::new(py, mock_models)?)?;
                fields.set_item("mock_source_names", PyTuple::new(py, mock_sources)?)?;
                fields.set_item("mock_seed_names", PyTuple::new(py, mock_seeds)?)?;
                fields.set_item("mock_dbt_ref_names", PyTuple::new(py, mock_dbt_refs)?)?;
                fields.set_item(
                    "mock_table_function_names",
                    PyTuple::new(py, mock_table_functions)?,
                )?;
                fields.set_item("expected_ctes", self.ctes(expected)?)?;
                fields.set_item("expected_model_names", PyTuple::new(py, expected_models)?)?;
                fields.set_item("assertion_ctes", self.ctes(assertions)?)?;
                fields.set_item("assertion_names", PyTuple::new(py, assertion_names)?)?;
                fields.set_item("assertion_target_model_names", PyTuple::new(py, targets)?)?;
                fields.set_item(
                    "read_helper_names",
                    PyTuple::new(py, reads.read_helper_names)?,
                )?;
                fields.set_item(
                    "reference_target_model_names",
                    PyTuple::new(py, reads.reference_target_model_names)?,
                )?;
                self.settings
                    .model_payload
                    .bind(py)
                    .call((), Some(&fields))?
            }
            SqlTestClassification::Direct {
                mode,
                helpers,
                actual,
                expected,
                ..
            } => {
                let fields: Bound<'py, PyDict> = PyDict::new(py);
                fields.set_item("mode", self.mode(mode)?)?;
                fields.set_item("helper_ctes", self.ctes(helpers)?)?;
                fields.set_item("actual_cte", self.cte(actual)?)?;
                fields.set_item("expected_cte", self.cte(expected)?)?;
                fields.set_item("tested_resource_names", PyTuple::new(py, &test.tested)?)?;
                self.settings
                    .direct_payload
                    .bind(py)
                    .call((), Some(&fields))?
            }
            SqlTestClassification::Authored { .. } => {
                return Err(self.error("native SQL test extraction returned an invalid kind"));
            }
        };
        let fields: Bound<'py, PyDict> = PyDict::new(py);
        fields.set_item("test_file", test_file.bind(py))?;
        fields.set_item("test_block", test_block.bind(py))?;
        fields.set_item("sql_body", &test.sql)?;
        fields.set_item("mode", self.mode(mode)?)?;
        fields.set_item("payload", payload)?;
        fields.set_item("declaration_usages", &test.usages)?;
        fields.set_item("parent_name", test_name)?;
        let block: &Bound<'py, PyAny> = test_block.bind(py);
        fields.set_item("parameter_schema", native_field(block, "parameters")?)?;
        if let Some((case, case_name, _)) = test.case {
            let case: &Bound<'py, PyAny> = case.bind(py);
            fields.set_item("case_name", case_name)?;
            fields.set_item("case_index", native_field(case, "case_index")?)?;
            fields.set_item("parameter_values", native_field(case, "values")?)?;
        }
        Ok(Some(
            self.settings
                .test_input
                .bind(py)
                .call((), Some(&fields))?
                .unbind(),
        ))
    }

    fn checked_helpers<T>(
        &self,
        outcome: Result<T, SqlTestCheckFailure<Infallible>>,
    ) -> PyResult<T> {
        outcome.map_err(|failure| match failure {
            SqlTestCheckFailure::Internal(message) | SqlTestCheckFailure::Scan(message) => {
                self.error(&message)
            }
            SqlTestCheckFailure::Matcher(never) => match never {},
        })
    }

    /// Report one P013 problem at its call, else its CTE header, else the test block.
    fn report_helper_problem(
        &self,
        file: &TestFileRow,
        block: &TestBlockRow,
        test_name: &str,
        problem: &SqlTestHelperProblem,
    ) -> PyResult<()> {
        let py: Python<'py> = self.py();
        let located: CteLocation = sql_test_cte_location(
            &file.contents,
            &block.sql_body,
            &problem.cte_name,
            problem.call.as_deref(),
        );
        let (line, column) = (located.line, located.column);
        let arguments: Bound<'py, PyDict> = PyDict::new(py);
        arguments.set_item("path", native_field(file.file.bind(py), "relative_path")?)?;
        arguments.set_item("line", line)?;
        arguments.set_item("column", column)?;
        arguments.set_item("end_line", located.end_line)?;
        arguments.set_item("end_column", located.end_column)?;
        let location: Bound<'py, PyAny> = self
            .settings
            .source_location
            .bind(py)
            .call((), Some(&arguments))?;
        let fields: Bound<'py, PyDict> = PyDict::new(py);
        fields.set_item("phase", "compile")?;
        fields.set_item("severity", "error")?;
        fields.set_item("code", HELPER_REFERENCE_CODE)?;
        fields.set_item("message", &problem.message)?;
        fields.set_item("resource_type", SQL_TEST_RESOURCE)?;
        fields.set_item("resource_name", test_name)?;
        fields.set_item("location", location)?;
        fields.set_item("help", &problem.help)?;
        let key: Bound<'py, PyTuple> = PyTuple::new(
            py,
            [
                HELPER_REFERENCE_CODE.to_owned(),
                file.relative_path.replace('\\', "/"),
                format!("{line:09}:{column:09}"),
                problem.message.clone(),
            ],
        )?;
        let arguments: Bound<'py, PyDict> = PyDict::new(py);
        arguments.set_item("key", key)?;
        arguments.set_item(
            "diagnostic",
            self.context
                .classes
                .compiler_diagnostic
                .call((), Some(&fields))?,
        )?;
        self.context
            .classes
            .report_diagnostic
            .call((), Some(&arguments))?;
        Ok(())
    }

    fn mode(&self, mode: &str) -> PyResult<Bound<'py, PyAny>> {
        self.settings
            .modes
            .bind(self.py())
            .get_item(mode)?
            .ok_or_else(|| self.error(&format!("unsupported SQL test mode '{mode}'")))
    }

    fn cte(&self, cte: &SqlTestCteText) -> PyResult<Bound<'py, PyAny>> {
        let fields: Bound<'py, PyDict> = PyDict::new(self.py());
        fields.set_item("name", &cte.0)?;
        fields.set_item("sql_body", &cte.1)?;
        self.settings
            .test_cte
            .bind(self.py())
            .call((), Some(&fields))
    }

    fn ctes(&self, ctes: &[SqlTestCteText]) -> PyResult<Bound<'py, PyTuple>> {
        let records: Vec<Bound<'py, PyAny>> = ctes
            .iter()
            .map(|cte| self.cte(cte))
            .collect::<PyResult<_>>()?;
        PyTuple::new(self.py(), records)
    }
}

/// `usage_visibility` for a test or scenario consumer: relationship grants, else every record.
fn test_visibility<'py>(records: &Bound<'py, PyAny>) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let all: Vec<Bound<'py, PyAny>> = records.try_iter()?.collect::<PyResult<_>>()?;
    let mut granted: Vec<Bound<'py, PyAny>> = Vec::new();
    for record in &all {
        if !native_field(record, "through")?.is_none() {
            granted.push(record.clone());
        }
    }
    Ok(if granted.is_empty() { all } else { granted })
}

/// `find_macro_call_names`: top-level macro call names, declaration calls excluded.
fn top_level_macro_names(sql: &str, syntax: &LexicalSyntax) -> Result<Vec<String>, &'static str> {
    Ok(macro_call_names(sql, false, Some(syntax))
        .map_err(sqlbuild_render::macro_calls::errors::ScanError::message)?
        .into_iter()
        .filter(|name| !DECLARATION_CALLS.contains(&name.as_str()))
        .collect())
}

/// The SQL with an omitted trailing `SELECT 1` completed, free of cursor intrinsics.
pub(crate) fn checked_body(
    model_loop: &NativeModelLoop,
    context: &LoopContext<'_, '_>,
    sql: &str,
    label: &str,
) -> PyResult<String> {
    let completed: String = match omitted_select_offset(sql, &model_loop.syntax) {
        OmittedSelect::At(offset) => {
            let mut characters = sql.chars();
            let head: String = characters.by_ref().take(offset).collect();
            format!("{head}{OMITTED_CEREMONIAL_SELECT}{}", characters.as_str())
        }
        OmittedSelect::Absent => sql.to_owned(),
    };
    match intrinsic_free(&completed, &model_loop.reserved_markers, label) {
        IntrinsicCheck::Free => Ok(completed),
        IntrinsicCheck::Rejected(message) => Err(compile_error(context.classes, &message, None)),
    }
}

/// `extract_assertion_target_model_names`: unique `__ref` targets of the assertions, in order.
pub(crate) fn assertion_targets(
    context: &LoopContext<'_, '_>,
    model_loop: &NativeModelLoop,
    assertions: &[SqlTestCteText],
    origin: FileOrigin<'_>,
) -> PyResult<Vec<String>> {
    let mut targets: Vec<String> = Vec::new();
    for cte in assertions {
        for reference in origin_references(context, &model_loop.syntax, &cte.1, origin)? {
            if reference.kind == REF_KIND {
                targets.push(reference.name);
            }
        }
    }
    Ok(unique(targets))
}

/// The names of `references` of `kind`, as `__source` reads list them.
pub(crate) fn source_names(references: &[SqlReference]) -> Vec<String> {
    references
        .iter()
        .filter(|reference| reference.kind == SOURCE_KIND)
        .map(|reference| reference.name.clone())
        .collect()
}

/// `_located_error`: the message with ` --> path:line:column` when the offset is known.
pub(crate) fn located_error(
    context: &LoopContext<'_, '_>,
    failure: &SqlTestExtractionFailure,
    file: (&str, &str),
    offset: Option<usize>,
) -> PyErr {
    let (relative_path, contents) = file;
    let message: String = match offset {
        Some(offset) => {
            let position: TextPosition = text_position(contents, offset);
            format!(
                "{}\n  --> {}:{}:{}",
                failure.message,
                relative_path.replace('\\', "/"),
                position.line,
                position.column
            )
        }
        None => failure.message.clone(),
    };
    helped_compile_error(context.classes, &message, failure.help.as_deref())
}

pub(crate) fn unique(items: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen: HashSet<String> = HashSet::new();
    items
        .into_iter()
        .filter(|item| seen.insert(item.clone()))
        .collect()
}
