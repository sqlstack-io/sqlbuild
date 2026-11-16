//! Public traits and type aliases of compiled views.

use crate::compiled_views::models::PyProjectHost;
use sqlbuild_project::models as project_models;

/// `(name, type)` of a declared column.
pub type DeclaredRow = (String, Option<String>);

/// `(output column, [(resource name, column name)])` of one lineage output.
pub type LineageInput = (String, Vec<(String, String)>);

/// A model's attached header schema; the host keeps its columns, audits and dynamic columns.
pub type SchemaRecord = project_models::SchemaRecord<PyProjectHost>;

/// What the model loop finished for one model beyond its facts.
pub type ModelInputRecord = project_models::ModelInputRecord<PyProjectHost>;

/// Declared keys by `kind:name` identity, in identity order.
pub type RelationKeys = Vec<(String, Vec<Vec<String>>)>;
