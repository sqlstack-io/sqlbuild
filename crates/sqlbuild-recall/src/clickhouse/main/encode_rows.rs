//! `encode_rows`: one table's rows as a `RowBinary` insert block.

use crate::clickhouse::_helpers::row_binary::{encoded, scaled};
use crate::clickhouse::constants::MICROS_DIGITS;
use crate::clickhouse::models::{ColumnEncoding, StoredColumn};
use crate::records::models::{RecallRow, RecallValue};
use crate::schema::constants::INGESTED_AT_COLUMN;

/// The column names and `RowBinary` block of `rows`, all of one table, written with `ingested_at`. Values for columns the table lacks are left out; columns the rows lack take their defaults.
pub fn encode_rows(
    rows: &[RecallRow],
    columns: &[StoredColumn],
    ingested_at_micros: i64,
) -> Result<(Vec<String>, Vec<u8>), String> {
    let Some(first) = rows.first() else {
        return Ok((Vec::new(), Vec::new()));
    };
    let written: Vec<&StoredColumn> = columns
        .iter()
        .filter(|column| column.name == INGESTED_AT_COLUMN || first.value(&column.name).is_some())
        .collect();
    let mut block: Vec<u8> = Vec::new();
    for row in rows {
        for column in &written {
            let value: RecallValue = if column.name == INGESTED_AT_COLUMN {
                RecallValue::Null
            } else {
                row.value(&column.name)
                    .cloned()
                    .unwrap_or(RecallValue::Null)
            };
            if column.name == INGESTED_AT_COLUMN {
                let precision: u64 = match column.encoding {
                    ColumnEncoding::DateTime64(precision) => precision,
                    _ => MICROS_DIGITS,
                };
                if column.nullable {
                    block.push(0);
                }
                block.extend_from_slice(&scaled(ingested_at_micros, precision).to_le_bytes());
                continue;
            }
            block = encoded(block, column, &value)?;
        }
    }
    Ok((
        written.iter().map(|column| column.name.clone()).collect(),
        block,
    ))
}
