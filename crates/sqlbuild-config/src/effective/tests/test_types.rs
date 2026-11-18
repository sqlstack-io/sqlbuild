use crate::effective::errors::EffectiveConfigError;
use crate::effective::models::FieldValue;

pub(super) type Pairs = &'static [(&'static str, i64)];
/// Builds one expected merged field value.
pub(super) type ExpectedField = fn() -> FieldValue<&'static str, i64>;

pub(super) struct TargetNameTestCase {
    pub(super) description: &'static str,
    pub(super) selected: Option<&'static str>,
    pub(super) local: Option<&'static str>,
    pub(super) default: Option<&'static str>,
    pub(super) expected_result: Result<Option<&'static str>, EffectiveConfigError<&'static str>>,
}

pub(super) struct ConnectionTestCase {
    pub(super) description: &'static str,
    pub(super) target_name: Option<&'static str>,
    pub(super) project_named: &'static [(&'static str, Pairs)],
    pub(super) local_named: &'static [(&'static str, Pairs)],
    pub(super) expected_result: Result<Pairs, EffectiveConfigError<&'static str>>,
}

pub(super) struct DefaultConnectionTestCase {
    pub(super) description: &'static str,
    pub(super) selected: bool,
    pub(super) project_names: &'static [&'static str],
    pub(super) local_names: &'static [&'static str],
    pub(super) expected_result: Result<Option<&'static str>, EffectiveConfigError<&'static str>>,
}

pub(super) struct MergeTestCase {
    pub(super) description: &'static str,
    /// Local fields to set, by name, with their values.
    pub(super) local: &'static [(&'static str, i64)],
    /// The merged value of each named field.
    pub(super) expected_result: &'static [(&'static str, ExpectedField)],
}

pub(super) struct UnionTestCase {
    pub(super) description: &'static str,
    pub(super) layers: &'static [Pairs],
    pub(super) expected_result: Pairs,
}

pub(super) struct MessageTestCase {
    pub(super) description: &'static str,
    pub(super) names: &'static [&'static str],
    pub(super) expected_message: &'static str,
}

pub(super) struct ConcurrencyTestCase {
    pub(super) description: &'static str,
    pub(super) requested: Option<i64>,
    pub(super) target: Option<(&'static str, i64)>,
    pub(super) settings: i64,
    pub(super) local_override: bool,
    pub(super) expected_limit: i64,
    pub(super) expected_source: &'static str,
}

pub(super) struct AuditFailureTestCase {
    pub(super) description: &'static str,
    pub(super) requested: Option<bool>,
    pub(super) target: Option<bool>,
    pub(super) settings: bool,
    pub(super) expected_ignored: bool,
}
