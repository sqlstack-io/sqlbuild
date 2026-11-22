use crate::audits::_helpers::parameters::{RenderStop, parameter_matches};
use crate::audits::models::{ArgumentValue, ParameterRenderStop};
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

/// `render_parameterized_sql`: raw and quoted arguments rendered into reusable SQL text.
pub fn render_parameterized_sql(
    sql: &str,
    arguments: &[(String, ArgumentValue)],
    (reject_unused, syntax): (bool, Option<&LexicalSyntax>),
) -> Result<String, ParameterRenderStop> {
    if reject_unused {
        let referenced: Vec<&str> = parameter_matches(sql)
            .into_iter()
            .map(|item| item.name)
            .collect();
        let mut unused: Vec<String> = arguments
            .iter()
            .map(|(name, _)| name.clone())
            .filter(|name| !referenced.contains(&name.as_str()))
            .collect();
        unused.sort();
        unused.dedup();
        if !unused.is_empty() {
            return Err(ParameterRenderStop::UnusedArguments(unused));
        }
    }
    crate::audits::_helpers::parameters::render_parameterized_sql(sql, arguments, syntax).map_err(
        |stop| match stop {
            RenderStop::MissingArgument(name) => ParameterRenderStop::MissingArgument(name),
            RenderStop::UnsupportedValue(name) => ParameterRenderStop::UnsupportedValue(name),
        },
    )
}
