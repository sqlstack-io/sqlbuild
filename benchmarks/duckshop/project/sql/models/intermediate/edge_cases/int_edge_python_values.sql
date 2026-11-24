MODEL (
  description """
    One row per distinct basket tag, read as the kind of payload value a project Python function
    hands to Snowflake: Python None in a VARIANT (JSON null, not SQL NULL), zoned and naive
    datetimes, Decimals written with an exponent and with a trailing zero, an integer past 2^53, a
    nested document, and an integer past 2^53 returned as FLOAT
  """,
  tags [edge_case],
  columns (
    kind (type VARCHAR, nullable false, audits [not_null, unique]),
    payload (type VARIANT),
    payload_is_sql_null (type BOOLEAN),
    payload_is_json_null (type BOOLEAN),
    payload_kind (type VARCHAR),
    payload_text (type VARCHAR),
    amount_as_float (type FLOAT),
  ),
);

WITH kinds AS (
  SELECT DISTINCT LOWER(TRIM(l.tag)) AS kind
  FROM __ref("stg_edge_basket_lines") AS l
  WHERE l.tag IS NOT NULL
),

values_out AS (
  SELECT
    k.kind,
    __udf("edge_payload_value")(k.kind) AS payload,
    __udf("edge_float_value")(9007199254740993) AS amount_as_float
  FROM kinds AS k
),

final AS (
  SELECT
    v.kind::VARCHAR AS kind,
    v.payload,
    (
      v.payload IS NULL
    )::BOOLEAN AS payload_is_sql_null,
    IS_NULL_VALUE(v.payload)::BOOLEAN AS payload_is_json_null,
    TYPEOF(v.payload)::VARCHAR AS payload_kind,
    TO_JSON(v.payload)::VARCHAR AS payload_text,
    v.amount_as_float::FLOAT AS amount_as_float
  FROM values_out AS v
)

SELECT *
FROM final
