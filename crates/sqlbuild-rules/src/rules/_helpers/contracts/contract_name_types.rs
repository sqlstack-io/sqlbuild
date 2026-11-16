use crate::constants::{BOOLEAN_TYPE, DATE_TYPE};
use crate::models::{Column, RulesConfig};
use sqlbuild_core::constants::TIMESTAMP_TYPE;

const BOOLEAN_COLUMN_RULE: &str = "SQBRCONTRACT102";
const TIMESTAMP_COLUMN_RULE: &str = "SQBRCONTRACT103";
const DATE_COLUMN_RULE: &str = "SQBRCONTRACT104";

pub(crate) fn finding(column: &Column, config: &RulesConfig) -> Option<(&'static str, String)> {
    let data_type = column.data_type.to_ascii_uppercase();
    if column.name.starts_with("is_")
        || column.name.starts_with("has_")
        || column.name.starts_with("can_")
    {
        let allowed_numeric = config
            .rule_option_enabled(BOOLEAN_COLUMN_RULE, "allow_numeric_indicators")
            && numeric_type(&data_type);
        return (data_type != BOOLEAN_TYPE && !allowed_numeric).then(|| {
            (
                BOOLEAN_COLUMN_RULE,
                format!(
                    "column {:?} implies BOOLEAN but is typed {data_type}",
                    column.name
                ),
            )
        });
    }
    if column.name.ends_with("_at")
        || column.name.ends_with("_ts")
        || column.name.ends_with("_timestamp")
    {
        let allowed_date = column.name.ends_with("_at")
            && data_type == DATE_TYPE
            && config.rule_option_enabled(TIMESTAMP_COLUMN_RULE, "allow_date_for_at");
        let allowed_epoch = numeric_type(&data_type)
            && config.rule_option_enabled(TIMESTAMP_COLUMN_RULE, "allow_numeric_epoch");
        let allowed_encoded = encoded_temporal_type(&data_type)
            && config.rule_option_enabled(TIMESTAMP_COLUMN_RULE, "allow_encoded_values");
        return (!data_type.contains(TIMESTAMP_TYPE)
            && !allowed_date
            && !allowed_epoch
            && !allowed_encoded)
            .then(|| {
                (
                    TIMESTAMP_COLUMN_RULE,
                    format!(
                        "column {:?} implies a timestamp but is typed {data_type}",
                        column.name
                    ),
                )
            });
    }
    if column.name.ends_with("_date") {
        let allowed_timestamp = data_type.contains(TIMESTAMP_TYPE)
            && config.rule_option_enabled(DATE_COLUMN_RULE, "allow_timestamps");
        let allowed_encoded = encoded_temporal_type(&data_type)
            && config.rule_option_enabled(DATE_COLUMN_RULE, "allow_encoded_values");
        return (data_type != DATE_TYPE && !allowed_timestamp && !allowed_encoded).then(|| {
            (
                DATE_COLUMN_RULE,
                format!(
                    "column {:?} implies DATE but is typed {data_type}",
                    column.name
                ),
            )
        });
    }
    None
}

fn numeric_type(data_type: &str) -> bool {
    [
        "BIGINT", "DECIMAL", "DOUBLE", "FLOAT", "INT", "NUMBER", "NUMERIC", "REAL", "SMALLINT",
    ]
    .iter()
    .any(|prefix| data_type.starts_with(prefix))
}

fn encoded_temporal_type(data_type: &str) -> bool {
    ["CHAR", "STRING", "TEXT", "VARCHAR", "VARIANT"]
        .iter()
        .any(|prefix| data_type.starts_with(prefix))
}
