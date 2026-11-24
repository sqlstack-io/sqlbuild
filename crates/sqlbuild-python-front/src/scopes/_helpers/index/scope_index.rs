//! The static scope index built natively from discovered inputs, as the Python scope models.

use crate::scopes::main::index::open_native_scope_index::open_native_scope_index;
use crate::scopes::models::NativeScopeIndex;
use std::collections::HashMap;

use pyo3::exceptions::PyKeyError;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use pyo3::{IntoPyObjectExt, pyfunction};
use sqlbuild_scopes::scope_index::main::macro_source_digest::macro_source_digest;
use sqlbuild_scopes::scope_index::main::normalize_scope_path::normalize_scope_path;
use sqlbuild_scopes::scope_index::main::rejected_path_message::rejected_path_message;
use sqlbuild_scopes::scope_index::models::{
    DeclarationIdentity, DeclarationInput, DeclarationKind, RejectedPath, ResourceIdentity,
    ResourceInput, ResourceKind, ResourceRoot, ScopeIndex, ScopeInputs, ScopeKind,
};

use crate::discovery::models::{ModelFileTexts, NativeModelFile};
use crate::scopes::_helpers::index::scope_lookup::scope_path_error;
use crate::scopes::_helpers::index::scope_objects::{field, optional_item, root_object, value_of};
use crate::scopes::_helpers::relationships::relationship_scan::{ScenarioRow, TestFileRow};
use crate::scopes::models::ScopeClasses;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_values::mapping_values;
use sqlbuild_python_base::records::main::fields::path_stem::path_stem;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;

/// Why `UnindexableScopeError` is raised; discovery only yields normalized project paths.
const UNINDEXABLE_SCOPE_MESSAGE: &str = "Native scope indexing could not place a discovered path or resource kind. Discovery always yields normalized project-relative paths, so this is a SQLBuild bug; please file an issue with the project layout";
const INSPECT_MODULE: &str = "inspect";
const CODE_TYPE_MODULE: &str = "types";
pub(crate) const PATHLIB_MODULE: &str = "pathlib";
const PRIVATE_ROOT: &str = "sql/models";
const MACRO_FALLBACK: &str = "sql/macros";
const ENUM_FALLBACK: &str = "sql/enums";
const CONSTANT_FALLBACK: &str = "sql/constants";
const COLLECTION_KINDS: [&str; 3] = ["list", "set", "object"];
const NULL_KIND: &str = "null";
/// Each resource kind's own ownership root, in the order Python sorts them by path.
const KIND_ROOTS: [(ResourceKind, &str); 5] = [
    (ResourceKind::Function, "sql/functions/sql"),
    (ResourceKind::Model, "sql/models"),
    (ResourceKind::Source, "sql/sources"),
    (ResourceKind::Scenario, "tests/scenarios"),
    (ResourceKind::Test, "tests/unit"),
];

/// The Python parts of one declaration record the native index does not hold.
struct DeclarationValues<'py> {
    identity: Bound<'py, PyAny>,
    line: i64,
    scope: Bound<'py, PyAny>,
    metadata: [Option<Bound<'py, PyAny>>; 3],
    dependencies: Vec<Bound<'py, PyAny>>,
}

/// Where one declaration lives: path, ownership root, root fallback and owning path.
struct Placement {
    path: String,
    ownership_root: Option<String>,
    root_fallback: String,
    owning_path: Option<String>,
}

/// The builder's walk of discovered inputs: native inputs and the Python values beside them.
pub(crate) struct Walk<'py> {
    classes: ScopeClasses<'py>,
    pub(crate) inputs: ScopeInputs,
    resource_identities: Vec<Bound<'py, PyAny>>,
    /// Each resource's kind and root, which pick its ownership root.
    resource_roots: Vec<(ResourceKind, ResourceRoot)>,
    declarations: Vec<DeclarationValues<'py>>,
    /// Whether every walked kind, scope and root parsed natively.
    pub(crate) indexable: bool,
}

fn text(value: &Bound<'_, PyAny>) -> PyResult<String> {
    value.str()?.extract()
}

fn optional_text(item: &Bound<'_, PyAny>, name: &str) -> PyResult<Option<String>> {
    optional_item(item, name)?
        .map(|value| text(&value))
        .transpose()
}

fn stem(path: &Bound<'_, PyAny>) -> PyResult<String> {
    path_stem(path)
}

/// `value or fallback` with Python truthiness.
fn or_else<'py>(
    value: Bound<'py, PyAny>,
    fallback: impl FnOnce() -> PyResult<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    if value.is_truthy()? {
        Ok(value)
    } else {
        fallback()
    }
}

/// `value` when it is a non-empty string.
fn header_text(value: &Bound<'_, PyAny>) -> Option<String> {
    value
        .extract::<String>()
        .into_iter()
        .find(|name| !name.is_empty())
}

/// `value if isinstance(value, str) and value else path.stem`.
fn name_or_stem(value: &Bound<'_, PyAny>, path: &Bound<'_, PyAny>) -> PyResult<String> {
    if let Ok(name) = value.extract::<String>()
        && !name.is_empty()
    {
        return Ok(name);
    }
    stem(path)
}

fn header_name<'py>(file: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    native_field(file, "header_values")?.call_method1("get", ("name",))
}

pub(crate) fn items<'py>(item: &Bound<'py, PyAny>, name: &str) -> PyResult<Vec<Bound<'py, PyAny>>> {
    native_field(item, name)?.try_iter()?.collect()
}

impl<'py> Walk<'py> {
    pub(crate) fn new(py: Python<'py>) -> PyResult<Self> {
        Ok(Self {
            classes: ScopeClasses::new(py)?,
            inputs: ScopeInputs::default(),
            resource_identities: Vec::new(),
            resource_roots: Vec::new(),
            declarations: Vec::new(),
            indexable: true,
        })
    }

    fn add_resource(
        &mut self,
        kind: ResourceKind,
        name: String,
        location: (&Bound<'py, PyAny>, ResourceRoot),
    ) -> PyResult<()> {
        let (path, root) = location;
        self.add_resource_at(kind, name, (text(path)?, root))
    }

    fn add_resource_at(
        &mut self,
        kind: ResourceKind,
        name: String,
        location: (String, ResourceRoot),
    ) -> PyResult<()> {
        let (path, root) = location;
        self.resource_identities
            .push(self.classes.model(RecordId::ResourceIdentity)?.call1((
                self.classes.member(RecordId::ResourceKind, kind.as_str())?,
                name.as_str(),
            ))?);
        self.resource_roots.push((kind, root));
        self.inputs.resources.push(ResourceInput {
            identity: ResourceIdentity { kind, name },
            path,
            root,
        });
        Ok(())
    }

    fn native_identity(&mut self, identity: &Bound<'py, PyAny>) -> PyResult<DeclarationIdentity> {
        let kind: String = value_of(identity, "kind")?;
        let owner: Option<ResourceIdentity> = match optional_item(identity, "owner")? {
            None => None,
            Some(owner) => {
                let owner_kind: String = value_of(&owner, "kind")?;
                match ResourceKind::parse(&owner_kind) {
                    Some(kind) => Some(ResourceIdentity {
                        kind,
                        name: field(&owner, "name")?,
                    }),
                    None => {
                        self.indexable = false;
                        None
                    }
                }
            }
        };
        let kind: DeclarationKind = DeclarationKind::parse(&kind).unwrap_or_else(|| {
            self.indexable = false;
            DeclarationKind::Macro
        });
        Ok(DeclarationIdentity {
            kind,
            name: field(identity, "name")?,
            owner,
        })
    }

    fn add_declaration(
        &mut self,
        values: DeclarationValues<'py>,
        placement: Placement,
    ) -> PyResult<()> {
        let identity: DeclarationIdentity = self.native_identity(&values.identity)?;
        let scope_text: String = native_field(&values.scope, "value")?.extract()?;
        let scope: ScopeKind = ScopeKind::parse(&scope_text).unwrap_or_else(|| {
            self.indexable = false;
            ScopeKind::Global
        });
        let dependencies: Vec<DeclarationIdentity> = values
            .dependencies
            .iter()
            .map(|item| self.native_identity(item))
            .collect::<PyResult<_>>()?;
        self.inputs.declarations.push(DeclarationInput {
            identity,
            path: placement.path,
            line: values.line,
            scope,
            ownership_root: placement.ownership_root,
            root_fallback: placement.root_fallback,
            owning_path: placement.owning_path,
            dependencies,
        });
        self.declarations.push(values);
        Ok(())
    }

    fn declaration_identity(
        &self,
        kind: DeclarationKind,
        name: &Bound<'py, PyAny>,
        owner: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        self.classes.model(RecordId::DeclarationIdentity)?.call1((
            self.classes
                .member(RecordId::DeclarationKind, kind.as_str())?,
            name,
            owner,
        ))
    }

    fn values(
        &self,
        identity: Bound<'py, PyAny>,
        scope: Bound<'py, PyAny>,
        metadata: [Option<Bound<'py, PyAny>>; 3],
    ) -> DeclarationValues<'py> {
        DeclarationValues {
            identity,
            line: 1,
            scope,
            metadata,
            dependencies: Vec::new(),
        }
    }

    fn enum_metadata(&self, declaration: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        let member_class: Bound<'py, PyAny> = self.classes.model(RecordId::EnumMemberMetadata)?;
        let members: Vec<Bound<'py, PyAny>> = items(declaration, "members")?
            .iter()
            .map(|member| member_class.call1((native_field(member, "name")?,)))
            .collect::<PyResult<_>>()?;
        self.classes.model(RecordId::EnumMetadata)?.call1((
            PyTuple::new(declaration.py(), members)?,
            native_field(declaration, "scalar_type")?,
        ))
    }

    fn constant_metadata(&self, declaration: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        let value: Bound<'py, PyAny> = native_field(declaration, "value")?;
        let kind: Bound<'py, PyAny> = native_field(&value, "kind")?;
        let kind_text: String = native_field(&kind, "value")?.extract()?;
        let is_collection: bool = COLLECTION_KINDS.contains(&kind_text.as_str());
        let payload: Bound<'py, PyAny> = native_field(&value, "value")?;
        let item_count: Option<usize> = if is_collection && payload.is_instance_of::<PyTuple>() {
            Some(payload.len()?)
        } else {
            None
        };
        let render_as: Option<Bound<'py, PyAny>> = optional_item(declaration, "render_as")?
            .map(|rendering| native_field(&rendering, "value"))
            .transpose()?;
        self.classes.model(RecordId::ConstantMetadata)?.call1((
            native_field(&native_field(declaration, "logical_type")?, "display_name")?,
            if is_collection {
                Some(native_field(&kind, "value")?)
            } else {
                None
            },
            item_count,
            kind_text == NULL_KIND,
            render_as,
        ))
    }

    pub(crate) fn add_models(&mut self, inputs: &Bound<'py, PyAny>) -> PyResult<()> {
        for model_file in items(inputs, "model_files")? {
            let header: Bound<'py, PyAny> = header_name(&model_file)?;
            let (model_name, path, folder): (String, String, String) =
                match model_file.cast::<NativeModelFile>() {
                    Ok(native) => {
                        let native: &NativeModelFile = native.get();
                        let texts: ModelFileTexts = native.path_texts(model_file.py())?;
                        let name: String = match header_text(&header) {
                            Some(name) => name,
                            None => texts.name,
                        };
                        (name, texts.relative_path, native.folder(model_file.py())?)
                    }
                    Err(_) => {
                        let path: Bound<'py, PyAny> = native_field(&model_file, "relative_path")?;
                        (
                            name_or_stem(&header, &path)?,
                            text(&path)?,
                            text(&native_field(&path, "parent")?)?,
                        )
                    }
                };
            self.add_resource_at(
                ResourceKind::Model,
                model_name.clone(),
                (path.clone(), ResourceRoot::Kind),
            )?;
            let enums: Vec<Bound<'py, PyAny>> = items(&model_file, "enum_declarations")?;
            let constants: Vec<Bound<'py, PyAny>> = items(&model_file, "constant_declarations")?;
            if enums.is_empty() && constants.is_empty() {
                continue;
            }
            let owner: Bound<'py, PyAny> =
                self.classes.model(RecordId::ResourceIdentity)?.call1((
                    self.classes
                        .member(RecordId::ResourceKind, ResourceKind::Model.as_str())?,
                    model_name,
                ))?;
            let private: Bound<'py, PyAny> = self
                .classes
                .member(RecordId::ScopeKind, ScopeKind::Private.as_str())?;
            for declaration in enums {
                let identity: Bound<'py, PyAny> = self.declaration_identity(
                    DeclarationKind::Enum,
                    &native_field(&declaration, "name")?,
                    Some(&owner),
                )?;
                let metadata = [None, Some(self.enum_metadata(&declaration)?), None];
                let values = self.values(identity, private.clone(), metadata);
                self.add_private(values, (&path, &folder))?;
            }
            for declaration in constants {
                let identity: Bound<'py, PyAny> = self.declaration_identity(
                    DeclarationKind::Constant,
                    &native_field(&declaration, "name")?,
                    Some(&owner),
                )?;
                let metadata = [None, None, Some(self.constant_metadata(&declaration)?)];
                let values = self.values(identity, private.clone(), metadata);
                self.add_private(values, (&path, &folder))?;
            }
        }
        Ok(())
    }

    fn add_private(
        &mut self,
        values: DeclarationValues<'py>,
        (path, folder): (&str, &str),
    ) -> PyResult<()> {
        self.add_declaration(
            values,
            Placement {
                path: path.to_owned(),
                ownership_root: Some(PRIVATE_ROOT.to_owned()),
                root_fallback: PRIVATE_ROOT.to_owned(),
                owning_path: Some(folder.to_owned()),
            },
        )
    }

    pub(crate) fn add_resources(&mut self, inputs: &Bound<'py, PyAny>) -> PyResult<()> {
        for test_file in items(inputs, "test_files")? {
            let path: Bound<'py, PyAny> = native_field(&test_file, "relative_path")?;
            for block in items(&test_file, "blocks")? {
                let name: Bound<'py, PyAny> = or_else(native_field(&block, "name")?, || {
                    native_field(&path, "stem")
                })?;
                self.add_resource(
                    ResourceKind::Test,
                    name.extract()?,
                    (&path, ResourceRoot::Kind),
                )?;
            }
        }
        for scenario in items(inputs, "scenario_files")? {
            self.add_resource(
                ResourceKind::Scenario,
                field(&scenario, "name")?,
                (
                    &native_field(&scenario, "relative_path")?,
                    ResourceRoot::Kind,
                ),
            )?;
        }
        for function in items(inputs, "sql_function_files")? {
            let path: Bound<'py, PyAny> = native_field(&function, "relative_path")?;
            let name: String = name_or_stem(&header_name(&function)?, &path)?;
            self.add_resource(ResourceKind::Function, name, (&path, ResourceRoot::Kind))?;
        }
        for function in items(inputs, "python_function_files")? {
            self.add_resource(
                ResourceKind::Function,
                stem(&native_field(&function, "file_path")?)?,
                (
                    &native_field(&function, "relative_path")?,
                    ResourceRoot::PythonFunction,
                ),
            )?;
        }
        for seed in items(inputs, "seed_files")? {
            self.add_resource(
                ResourceKind::Seed,
                stem(&native_field(&seed, "file_path")?)?,
                (&native_field(&seed, "relative_path")?, ResourceRoot::Seed),
            )?;
        }
        for source_file in items(inputs, "source_files")? {
            let path: Bound<'py, PyAny> = native_field(&source_file, "relative_path")?;
            for source in items(&source_file, "source_entries")? {
                self.add_resource(
                    ResourceKind::Source,
                    field(&source, "name")?,
                    (&path, ResourceRoot::Kind),
                )?;
            }
        }
        Ok(())
    }

    fn file_placement(file: &Bound<'py, PyAny>, root_fallback: String) -> PyResult<Placement> {
        Ok(Placement {
            path: text(&native_field(file, "relative_path")?)?,
            ownership_root: optional_text(file, "ownership_root")?,
            root_fallback,
            owning_path: optional_text(file, "owning_path")?,
        })
    }

    pub(crate) fn add_declaration_files(&mut self, inputs: &Bound<'py, PyAny>) -> PyResult<()> {
        for enum_file in items(inputs, "enum_files")? {
            for declaration in items(&enum_file, "declarations")? {
                let identity: Bound<'py, PyAny> = self.declaration_identity(
                    DeclarationKind::Enum,
                    &native_field(&declaration, "name")?,
                    None,
                )?;
                let metadata = [None, Some(self.enum_metadata(&declaration)?), None];
                let values =
                    self.values(identity, native_field(&enum_file, "scope_kind")?, metadata);
                let placement = Self::file_placement(&enum_file, ENUM_FALLBACK.to_owned())?;
                self.add_declaration(values, placement)?;
            }
        }
        for constant_file in items(inputs, "constant_files")? {
            for declaration in items(&constant_file, "declarations")? {
                let identity: Bound<'py, PyAny> = self.declaration_identity(
                    DeclarationKind::Constant,
                    &native_field(&declaration, "name")?,
                    None,
                )?;
                let metadata = [None, None, Some(self.constant_metadata(&declaration)?)];
                let values = self.values(
                    identity,
                    native_field(&constant_file, "scope_kind")?,
                    metadata,
                );
                let placement = Self::file_placement(&constant_file, CONSTANT_FALLBACK.to_owned())?;
                self.add_declaration(values, placement)?;
            }
        }
        Ok(())
    }

    fn add_named(
        &mut self,
        item: &Bound<'py, PyAny>,
        kind: DeclarationKind,
        name: &Bound<'py, PyAny>,
    ) -> PyResult<()> {
        let identity: Bound<'py, PyAny> = self.declaration_identity(kind, name, None)?;
        let values = self.values(
            identity,
            native_field(item, "scope_kind")?,
            [None, None, None],
        );
        let fallback: Bound<'py, PyAny> = or_else(native_field(item, "declaration_root")?, || {
            native_field(&native_field(item, "relative_path")?, "parent")
        })?;
        let placement = Self::file_placement(item, text(&fallback)?)?;
        self.add_declaration(values, placement)
    }

    pub(crate) fn add_named_declarations(&mut self, inputs: &Bound<'py, PyAny>) -> PyResult<()> {
        let audit_kind: String = DeclarationKind::Audit.as_str().to_owned();
        for audit_file in items(inputs, "audit_files")? {
            let stem_name: Bound<'py, PyAny> =
                native_field(&native_field(&audit_file, "relative_path")?, "stem")?;
            if value_of(&audit_file, "declaration_kind")? == audit_kind {
                self.add_named(&audit_file, DeclarationKind::Audit, &stem_name)?;
                continue;
            }
            for block in items(&audit_file, "blocks")? {
                let name: Bound<'py, PyAny> =
                    or_else(native_field(&block, "name")?, || Ok(stem_name.clone()))?;
                self.add_named(&audit_file, DeclarationKind::SingularAudit, &name)?;
            }
        }
        for schema_file in items(inputs, "model_schema_files")? {
            for declaration in items(&schema_file, "declarations")? {
                self.add_named(
                    &schema_file,
                    DeclarationKind::Schema,
                    &native_field(&declaration, "name")?,
                )?;
            }
        }
        for hook_file in items(inputs, "sql_hook_files")? {
            self.add_named(
                &hook_file,
                DeclarationKind::SqlHook,
                &native_field(&hook_file, "name")?,
            )?;
        }
        for hook in items(inputs, "hook_functions")? {
            self.add_named(
                &hook,
                DeclarationKind::PythonHook,
                &native_field(&hook, "name")?,
            )?;
        }
        Ok(())
    }

    fn macro_values(
        &self,
        macro_file: Option<&Bound<'py, PyAny>>,
        source: (&Bound<'py, PyAny>, i64, Bound<'py, PyAny>),
    ) -> PyResult<(DeclarationValues<'py>, Placement)> {
        let (export, line, metadata) = source;
        let identity: Bound<'py, PyAny> = self.declaration_identity(
            DeclarationKind::Macro,
            &native_field(export, "name")?,
            None,
        )?;
        let scope: Bound<'py, PyAny> = match macro_file {
            Some(file) => native_field(file, "scope_kind")?,
            None => self
                .classes
                .member(RecordId::ScopeKind, ScopeKind::Global.as_str())?,
        };
        let dependencies: Vec<Bound<'py, PyAny>> = items(export, "dependencies")?;
        let placement: Placement = Placement {
            path: text(&native_field(export, "relative_path")?)?,
            ownership_root: match macro_file {
                Some(file) => optional_text(file, "ownership_root")?,
                None => None,
            },
            root_fallback: MACRO_FALLBACK.to_owned(),
            owning_path: match macro_file {
                Some(file) => optional_text(file, "owning_path")?,
                None => None,
            },
        };
        Ok((
            DeclarationValues {
                identity,
                line,
                scope,
                metadata: [Some(metadata), None, None],
                dependencies,
            },
            placement,
        ))
    }

    fn loaded_metadata(&self, loaded: &Bound<'py, PyAny>) -> PyResult<(i64, Bound<'py, PyAny>)> {
        let py: Python<'py> = loaded.py();
        let function: Bound<'py, PyAny> = native_field(loaded, "function")?;
        let parameters: Vec<Bound<'py, PyAny>> = py
            .import(INSPECT_MODULE)?
            .getattr("signature")?
            .call1((&function,))?
            .getattr("parameters")?
            .try_iter()?
            .collect::<PyResult<_>>()?;
        let raw_source: String = field(loaded, "raw_source")?;
        let metadata: Bound<'py, PyAny> = self.classes.model(RecordId::MacroMetadata)?.call1((
            PyTuple::new(py, parameters)?,
            native_field(loaded, "dependencies")?,
            macro_source_digest(&raw_source),
        ))?;
        let code_type: Bound<'py, PyAny> = py.import(CODE_TYPE_MODULE)?.getattr("CodeType")?;
        let line: i64 = match function.getattr_opt("__code__")? {
            Some(code) if code.is_instance(&code_type)? => {
                native_field(&code, "co_firstlineno")?.extract()?
            }
            _ => 1,
        };
        Ok((line, metadata))
    }

    pub(crate) fn add_macros(
        &mut self,
        inputs: &Bound<'py, PyAny>,
        macros: (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    ) -> PyResult<()> {
        let (loaded_macros, static_macros) = macros;
        let py: Python<'py> = inputs.py();
        let mut macro_files: HashMap<String, Bound<'py, PyAny>> = HashMap::new();
        for macro_file in items(inputs, "macro_files")? {
            let key: String = normalized(py, &native_field(&macro_file, "relative_path")?)?;
            macro_files.insert(key, macro_file);
        }
        for loaded in mapping_values(loaded_macros)?.try_iter()? {
            let loaded: Bound<'py, PyAny> = loaded?;
            let key: String = normalized(py, &native_field(&loaded, "relative_path")?)?;
            let (line, metadata) = self.loaded_metadata(&loaded)?;
            let (values, placement) =
                self.macro_values(macro_files.get(&key), (&loaded, line, metadata))?;
            self.add_declaration(values, placement)?;
        }
        for export in static_macros.try_iter()? {
            let export: Bound<'py, PyAny> = export?;
            let key: String = normalized(py, &native_field(&export, "relative_path")?)?;
            let metadata: Bound<'py, PyAny> =
                self.classes.model(RecordId::MacroMetadata)?.call1((
                    native_field(&export, "parameters")?,
                    native_field(&export, "dependencies")?,
                    native_field(&export, "source_digest")?,
                ))?;
            let (values, placement) = self.macro_values(
                macro_files.get(&key),
                (&export, field(&export, "line")?, metadata),
            )?;
            self.add_declaration(values, placement)?;
        }
        Ok(())
    }
}

/// Python's scope `normalize_path` of a path; a rejected path raises `InvalidScopePathError`.
fn normalized(py: Python<'_>, path: &Bound<'_, PyAny>) -> PyResult<String> {
    let path: String = text(path)?;
    normalize_scope_path(&path).map_err(|failure| {
        scope_path_error(py, rejected_path_message(&RejectedPath { failure, path }))
            .unwrap_or_else(|error| error)
    })
}

pub(crate) fn unindexable(py: Python<'_>) -> PyResult<PyErr> {
    Ok(PyErr::from_value(
        error_class(py, ErrorId::UnindexableScope)?.call1((UNINDEXABLE_SCOPE_MESSAGE,))?,
    ))
}

fn resource_root<'py>(
    classes: &ScopeClasses<'py>,
    kind: ResourceKind,
    root: ResourceRoot,
) -> PyResult<Bound<'py, PyAny>> {
    let (path, root_kind, resource_kind): (&str, &str, ResourceKind) = match root {
        ResourceRoot::Seed => ("sql/seeds", "global", ResourceKind::Seed),
        ResourceRoot::PythonFunction => ("sql/functions/python", "global", ResourceKind::Function),
        ResourceRoot::Kind => match KIND_ROOTS.iter().find(|(candidate, _)| *candidate == kind) {
            Some((_, path)) => (path, "resource", kind),
            None => {
                return Err(PyKeyError::new_err(
                    classes
                        .member(RecordId::ResourceKind, kind.as_str())?
                        .unbind(),
                ));
            }
        },
    };
    classes.model(RecordId::OwnershipRoot)?.call1((
        path,
        classes.member(RecordId::OwnershipRootKind, root_kind)?,
        classes.member(RecordId::ResourceKind, resource_kind.as_str())?,
    ))
}

pub(crate) fn materialise<'py>(walk: Walk<'py>, native: ScopeIndex) -> PyResult<NativeScopeIndex> {
    let classes: &ScopeClasses<'py> = &walk.classes;
    let py: Python<'py> = classes.py();
    let mut resources: Vec<Bound<'py, PyAny>> = Vec::with_capacity(native.resources.len());
    for ((record, (kind, root)), identity) in native
        .resources
        .iter()
        .zip(&walk.resource_roots)
        .zip(&walk.resource_identities)
    {
        resources.push(classes.model(RecordId::ResourceRecord)?.call1((
            identity,
            record.path.as_str(),
            resource_root(classes, *kind, *root)?,
        ))?);
    }
    let mut declarations: Vec<Bound<'py, PyAny>> = Vec::with_capacity(native.declarations.len());
    for (record, values) in native.declarations.iter().zip(&walk.declarations) {
        let [macro_metadata, enum_metadata, constant_metadata] = &values.metadata;
        let arguments: Vec<Bound<'py, PyAny>> = vec![
            values.identity.clone(),
            record.path.as_str().into_bound_py_any(py)?,
            values.line.into_bound_py_any(py)?,
            1_i64.into_bound_py_any(py)?,
            values.scope.clone(),
            root_object(classes, &record.ownership_root)?,
            record.owning_path.as_deref().into_bound_py_any(py)?,
            macro_metadata.into_bound_py_any(py)?,
            enum_metadata.into_bound_py_any(py)?,
            constant_metadata.into_bound_py_any(py)?,
        ];
        declarations.push(
            classes
                .model(RecordId::DeclarationRecord)?
                .call1(PyTuple::new(py, arguments)?)?,
        );
    }
    let usage_class: Bound<'py, PyAny> = classes.model(RecordId::UsageRecord)?;
    let dependency_kind: Bound<'py, PyAny> =
        classes.member(RecordId::UsageKind, "declaration_dependency")?;
    let mut usages: Vec<Bound<'py, PyAny>> = Vec::with_capacity(native.usages.len());
    for usage in &native.usages {
        usages.push(usage_class.call1((
            native_field(&declarations[usage.consumer], "identity")?,
            &walk.declarations[usage.consumer].dependencies[usage.dependency],
            &dependency_kind,
        ))?);
    }
    let diagnostic_class: Bound<'py, PyAny> = classes.model(RecordId::ScopeDiagnostic)?;
    let mut diagnostics: Vec<Bound<'py, PyAny>> = Vec::with_capacity(native.diagnostics.len());
    for diagnostic in &native.diagnostics {
        let keywords: Bound<'py, PyDict> = PyDict::new(py);
        keywords.set_item("path", diagnostic.path.as_deref())?;
        keywords.set_item("line", diagnostic.line)?;
        keywords.set_item("column", diagnostic.column)?;
        keywords.set_item(
            "declaration",
            diagnostic
                .declaration
                .map(|position| native_field(&declarations[position], "identity"))
                .transpose()?,
        )?;
        keywords.set_item(
            "resource",
            diagnostic
                .resource
                .map(|position| native_field(&resources[position], "identity"))
                .transpose()?,
        )?;
        diagnostics.push(diagnostic_class.call(
            (
                classes.member(RecordId::ScopeDiagnosticCode, diagnostic.code.as_str())?,
                diagnostic.message.as_str(),
            ),
            Some(&keywords),
        )?);
    }
    let roots: Vec<Bound<'py, PyAny>> = KIND_ROOTS
        .iter()
        .map(|(kind, path)| {
            let keywords: Bound<'py, PyDict> = PyDict::new(py);
            keywords.set_item("path", *path)?;
            keywords.set_item(
                "resource_kind",
                classes.member(RecordId::ResourceKind, kind.as_str())?,
            )?;
            classes
                .model(RecordId::OwnershipRoot)?
                .call((), Some(&keywords))
        })
        .collect::<PyResult<_>>()?;
    let completeness: Bound<'py, PyAny> = {
        let keywords: Bound<'py, PyDict> = PyDict::new(py);
        for section in [
            "runtime_usage",
            "relationships",
            "placement",
            "promotion_impact",
        ] {
            keywords.set_item(section, false)?;
        }
        classes
            .model(RecordId::ScopeCompleteness)?
            .call((), Some(&keywords))?
    };
    let ordered =
        |records: &[Bound<'py, PyAny>], order: &[usize]| -> PyResult<Bound<'py, PyTuple>> {
            PyTuple::new(py, order.iter().map(|position| records[*position].clone()))
        };
    let index: Bound<'py, PyAny> = classes.model(RecordId::ScopeIndex)?.call1((
        PyTuple::new(py, roots)?,
        ordered(&resources, &native.resource_order)?,
        ordered(&declarations, &native.declaration_order)?,
        PyTuple::new(py, usages)?,
        PyTuple::empty(py),
        PyTuple::empty(py),
        PyTuple::empty(py),
        PyTuple::new(py, diagnostics)?,
        completeness,
    ))?;
    Ok(NativeScopeIndex {
        native,
        declarations: declarations.into_iter().map(Bound::unbind).collect(),
        index: index.unbind(),
        grants: PyTuple::empty(py).unbind(),
    })
}

pub(crate) fn test_file_row(test_file: &Bound<'_, PyAny>) -> PyResult<TestFileRow> {
    let path: Bound<'_, PyAny> = native_field(test_file, "relative_path")?;
    let mut blocks: Vec<(String, String, String)> = Vec::new();
    for block in items(test_file, "blocks")? {
        let name: Bound<'_, PyAny> = or_else(native_field(&block, "name")?, || {
            native_field(&path, "stem")
        })?;
        blocks.push((
            name.extract()?,
            field(&block, "sql_body")?,
            value_of(&block, "mode")?,
        ));
    }
    Ok((text(&path)?, blocks))
}

pub(crate) fn scenario_row(scenario: &Bound<'_, PyAny>) -> PyResult<ScenarioRow> {
    Ok((
        field(scenario, "name")?,
        text(&native_field(scenario, "relative_path")?)?,
        field(scenario, "sql_body")?,
    ))
}

/// Python's `build_scope_index`: the natively built, partially valid index of discovered inputs.
#[pyfunction]
#[pyo3(
    name = "build_scope_index",
    signature = (*, discovered_inputs, loaded_macros = None, static_macros = None)
)]
pub(crate) fn build_scope_index_binding(
    py: Python<'_>,
    discovered_inputs: &Bound<'_, PyAny>,
    loaded_macros: Option<Bound<'_, PyAny>>,
    static_macros: Option<Bound<'_, PyAny>>,
) -> PyResult<Py<PyAny>> {
    let loaded_macros: Bound<'_, PyAny> = match loaded_macros {
        Some(loaded_macros) if !loaded_macros.is_none() => loaded_macros,
        _ => PyDict::new(py).into_any(),
    };
    let native: NativeScopeIndex =
        open_native_scope_index(py, discovered_inputs, &loaded_macros, static_macros)?;
    Ok(native.index.clone_ref(py))
}
