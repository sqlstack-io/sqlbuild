use crate::audits::_helpers::parameters::RenderStop;
use crate::audits::_helpers::parameters::render_parameterized_sql;
use crate::audits::models::{
    ArgumentValue, AuditAttachment, AuditRendering, PolicySource, RenderedAudit,
};

/// Arguments shared by the parameter rendering cases.
pub(super) fn order_arguments() -> Vec<(String, ArgumentValue)> {
    vec![
        (
            "label".to_owned(),
            ArgumentValue::Text("O'Brien@orders".to_owned()),
        ),
        (
            "column".to_owned(),
            ArgumentValue::Text("status".to_owned()),
        ),
        (
            "values".to_owned(),
            ArgumentValue::List(vec![
                ArgumentValue::Text("it's".to_owned()),
                ArgumentValue::Number("2".to_owned()),
                ArgumentValue::Null,
                ArgumentValue::Boolean(true),
            ]),
        ),
    ]
}

pub(super) fn rendered_sql(sql: &str) -> Result<String, RenderStop> {
    render_parameterized_sql(sql, &order_arguments(), None)
}

/// A violations audit over `@column` with the given explicit arguments and policies.
pub(super) fn attachment(
    explicit: Vec<(String, ArgumentValue)>,
    severity: Option<&str>,
    run_scope: Option<&str>,
) -> AuditAttachment {
    AuditAttachment {
        owner_label: "models/schema.yml".to_owned(),
        definition_name: "accepted_values".to_owned(),
        definition_path: None,
        sql_body: "SELECT * FROM t WHERE @column NOT IN (@'values')".to_owned(),
        evidence_sql: Some("SELECT @column FROM t".to_owned()),
        implicit_arguments: vec![("column".to_owned(), "status".to_owned())],
        explicit_arguments: explicit,
        has_thresholds: false,
        threshold_error: false,
        instance_severity: severity.map(str::to_owned),
        default_severity: None,
        instance_run_scope: run_scope.map(str::to_owned),
        default_run_scope: None,
    }
}

/// The rendering of `attachment()` SQL: `sql_body`, the fixed evidence and these policies.
pub(super) fn rendered(
    sql_body: &str,
    policies: Result<(&'static str, PolicySource), &str>,
) -> AuditRendering {
    AuditRendering::Rendered(RenderedAudit {
        sql_body: sql_body.to_owned(),
        evidence_sql: Some("SELECT status FROM t".to_owned()),
        policies: policies.map_err(str::to_owned),
    })
}

pub(super) fn failed(message: &str) -> AuditRendering {
    AuditRendering::Failed(message.to_owned())
}
