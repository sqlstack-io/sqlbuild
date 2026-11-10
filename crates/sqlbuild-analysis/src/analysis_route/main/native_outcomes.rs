//! Native outcomes for a wave of models, one analysis per share key two models hold.

use rayon::ThreadPool;
use rayon::iter::{IntoParallelIterator, ParallelIterator};

use crate::analysis_route::_helpers::analysis::native_sharing::{
    NativeMember, analysed_members, native_member,
};
use crate::analysis_route::models::ShareCounts;
use crate::analysis_route::types::{NativeDialect, RelationsOf};
use crate::assembly::analysis_session::models::{ModelOutcome, ModelRequest};
use crate::assembly::analysis_session::types::Shapes;

/// Each model's outcome and failure over its schema, and how many models shared an analysis.
pub(crate) fn native_outcomes(
    dialect: &dyn NativeDialect,
    pool: &ThreadPool,
    models: Vec<(&ModelRequest, &Shapes)>,
    (relations_of, settings): (RelationsOf<'_>, (&[String], bool)),
) -> (Vec<(ModelOutcome, Option<String>)>, ShareCounts) {
    let members: Vec<NativeMember<'_>> = pool.install(|| {
        models
            .into_par_iter()
            .map(|(model, schema)| native_member(dialect, model, schema.clone(), relations_of))
            .collect()
    });
    analysed_members(dialect, pool, members, (relations_of, settings))
}
