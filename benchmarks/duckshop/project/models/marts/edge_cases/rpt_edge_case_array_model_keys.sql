MODEL (
  description """
    One row per key of each edge-case model with an ARRAY, OBJECT or VARIANT output that reads a
    construct translation tools refuse, so that every such model has a downstream reader
  """,
  tags [edge_case],
  columns (
    model_key_id (type VARCHAR, nullable false, audits [not_null, unique]),
    model_name (type VARCHAR),
    model_key (type VARCHAR),
  ),
);

WITH model_keys AS (
  SELECT 'int_edge_callback_wall_time_lists' AS model_name, r.merchant_zone::VARCHAR AS model_key
  FROM __ref("int_edge_callback_wall_time_lists") AS r
),

final AS (
  SELECT
    k.model_name || '~' || k.model_key AS model_key_id,
    k.model_name,
    k.model_key
  FROM model_keys AS k
)

SELECT *
FROM final
