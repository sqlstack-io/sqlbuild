//! Indexed exact-node/destination reads, shared by evidence, definition comparison and the catalog.

use std::collections::{BTreeSet, HashMap};

use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::main::{decode_row::decode_row, decompress::decompress, query::query};
use crate::store::models::{ControlTransaction, Parameter};
use crate::warehouse_metadata::models::{FingerprintObservation, FingerprintQuery};

/// Nodes read per statement; well under both stores' bound-parameter limits.
const NODES_PER_STATEMENT: usize = 100;

/// A node's newest build, its recorded metadata JSON and the hash of its recorded definition text.
type LatestBuild = Option<(FingerprintObservation, String, String)>;

pub(crate) async fn latest_fingerprint(
    transaction: ControlTransaction,
    (location_id, target, options): (Uuid, &str, &FingerprintQuery),
) -> Result<(ControlTransaction, LatestBuild), ControlError> {
    let (transaction, mut builds) = latest_fingerprints(
        transaction,
        (location_id, target, std::slice::from_ref(options)),
    )
    .await?;
    Ok((transaction, builds.pop().flatten()))
}

/// Each query's newest build, in order; NULL-namespace shapes share exact-seek statements.
pub(crate) async fn latest_fingerprints(
    mut transaction: ControlTransaction,
    (location_id, target, queries): (Uuid, &str, &[FingerprintQuery]),
) -> Result<(ControlTransaction, Vec<LatestBuild>), ControlError> {
    if queries
        .iter()
        .any(|options| options.node_type.trim().is_empty() || options.node_name.trim().is_empty())
    {
        return Err(ControlError::Invalid(
            "fingerprint reads require an exact node".into(),
        ));
    }
    let mut builds: Vec<LatestBuild> = vec![None; queries.len()];
    let namespace = |options: &FingerprintQuery| {
        [
            options.database.is_some(),
            options.schema.is_some(),
            options.name.is_some(),
        ]
    };
    let mut shapes: Vec<[bool; 3]> = queries.iter().map(namespace).collect();
    shapes.sort_unstable();
    shapes.dedup();
    for shape in shapes {
        let positions: Vec<usize> = (0..queries.len())
            .filter(|position| namespace(&queries[*position]) == shape)
            .collect();
        for chunk in positions.chunks(NODES_PER_STATEMENT) {
            let mut parameters = vec![Parameter::Id(location_id), Parameter::Text(target.into())];
            let mut rows: Vec<String> = Vec::with_capacity(chunk.len());
            for position in chunk {
                let options = &queries[*position];
                let first = parameters.len() + 1;
                parameters.push(Parameter::Integer(i64::try_from(*position).map_err(
                    |_| ControlError::Invalid("too many fingerprint reads".into()),
                )?));
                parameters.push(Parameter::Text(options.node_type.clone()));
                parameters.push(Parameter::Text(options.node_name.clone()));
                for value in [&options.database, &options.schema, &options.name]
                    .into_iter()
                    .flatten()
                {
                    parameters.push(Parameter::Text(value.clone()));
                }
                rows.push(format!(
                    "({})",
                    (first..=parameters.len())
                        .map(|index| format!("${index}"))
                        .collect::<Vec<String>>()
                        .join(",")
                ));
            }
            let mut column = 3;
            let predicates: Vec<String> = ["target_database", "target_schema", "target_name"]
                .into_iter()
                .zip(shape)
                .map(|(name, present)| {
                    if present {
                        column += 1;
                        format!("g.{name}=w.column{column}")
                    } else {
                        format!("g.{name} IS NULL")
                    }
                })
                .collect();
            let found;
            (transaction, found) = query(transaction,
                &format!("SELECT w.column1 AS position,f.event_id,f.observed_at,f.source_event_id,f.record_json ->> 'run_id' AS run_id, \
                    f.record_json ->> 'definition_hash' AS definition_hash,f.record_json ->> 'version_hash' AS version_hash, \
                    f.record_json ->> 'schema_fingerprint' AS schema_fingerprint,f.record_json ->> 'metadata_json' AS metadata_json, \
                    f.definition_text_hash \
                    FROM (VALUES {}) AS w JOIN sqb_warehouse_fingerprints f ON f.event_id=(SELECT g.event_id FROM sqb_warehouse_fingerprints g \
                    WHERE g.location_id=$1 AND g.target=$2 AND g.node_type=w.column2 AND g.node_name=w.column3 AND {} \
                    ORDER BY g.target_database,g.target_schema,g.target_name,g.observed_at DESC,g.event_id DESC LIMIT 1)",
                    rows.join(","), predicates.join(" AND ")), parameters).await?;
            for mut row in found {
                let position: usize =
                    serde_json::from_value(row.remove("position").unwrap_or_default())
                        .map_err(|error| ControlError::Invalid(error.to_string()))?;
                let metadata = row
                    .remove("metadata_json")
                    .and_then(|value| value.as_str().map(str::to_owned))
                    .unwrap_or_default();
                let text_hash = row
                    .remove("definition_text_hash")
                    .and_then(|value| value.as_str().map(str::to_owned))
                    .unwrap_or_default();
                builds[position] = Some((decode_row(row)?, metadata, text_hash));
            }
        }
    }
    Ok((transaction, builds))
}

/// The recorded definition texts with these hashes; a hash with no stored text is left out.
pub(crate) async fn definition_texts(
    mut transaction: ControlTransaction,
    hashes: &BTreeSet<String>,
) -> Result<(ControlTransaction, HashMap<String, String>), ControlError> {
    let hashes: Vec<&String> = hashes.iter().collect();
    let mut texts: HashMap<String, String> = HashMap::new();
    for chunk in hashes.chunks(NODES_PER_STATEMENT) {
        let placeholders: Vec<String> =
            (1..=chunk.len()).map(|index| format!("${index}")).collect();
        let found;
        (transaction, found) = query(
            transaction,
            &format!(
                "SELECT text_hash, body FROM sqb_source_texts WHERE text_hash IN ({})",
                placeholders.join(",")
            ),
            chunk
                .iter()
                .map(|hash| Parameter::Text((*hash).clone()))
                .collect(),
        )
        .await?;
        for mut row in found {
            let hash = row
                .remove("text_hash")
                .and_then(|value| value.as_str().map(str::to_owned))
                .unwrap_or_default();
            let body = decompress(&row.remove("body").unwrap_or_default())?;
            if let Ok(text) = String::from_utf8(body) {
                texts.insert(hash, text);
            }
        }
    }
    Ok((transaction, texts))
}
