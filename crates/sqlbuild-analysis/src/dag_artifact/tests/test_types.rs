use crate::assembly::project::types::ObjectKey;
use crate::dag_artifact::errors::DagArtifactError;
use crate::dag_artifact::models::{DagProjectInput, DagPythonGraphInput};

pub(crate) struct BuildDagJsonTestCase {
    pub(crate) description: &'static str,
    pub(crate) project: DagProjectInput,
    pub(crate) python: Option<DagPythonGraphInput>,
    pub(crate) upstream: Vec<(ObjectKey, Vec<ObjectKey>)>,
    pub(crate) expected_json: Result<&'static str, DagArtifactError>,
}

pub(crate) struct AssetPlansTestCase {
    pub(crate) description: &'static str,
    pub(crate) upstream_asset_kinds_only: bool,
    pub(crate) expected_plans: Vec<(usize, Vec<usize>)>,
}

pub(crate) struct CheckPlansTestCase {
    pub(crate) description: &'static str,
    pub(crate) filter: crate::dag_artifact::models::CheckKindFilter,
    pub(crate) expected_plans: Vec<(usize, bool, Vec<usize>)>,
}

pub(crate) struct SpecKindsTestCase {
    pub(crate) description: &'static str,
    pub(crate) kind: &'static str,
    pub(crate) materialization_type: &'static str,
    pub(crate) expected_kinds: Vec<&'static str>,
}

pub(crate) struct DagEdgeTestCase {
    pub(crate) description: &'static str,
    pub(crate) project: DagProjectInput,
    pub(crate) python: DagPythonGraphInput,
    /// `(from_id, to_id)` of every edge, sorted.
    pub(crate) expected_edges: &'static [(&'static str, &'static str)],
}
