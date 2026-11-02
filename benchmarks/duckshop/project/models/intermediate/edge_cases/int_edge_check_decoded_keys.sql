MODEL (
  description """
    One row per reconciliation check with its key text decoded from Base64 as binary, and the
    bytes written back as hex
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    decoded_key_hex (type VARCHAR),
  ),
);

WITH decoded AS (
  SELECT
    a.check_id,
    TRY_BASE64_DECODE_BINARY(a.key_text) AS decoded_key
  FROM __ref("stg_edge_amount_checks") AS a
),

final AS (
  SELECT
    d.check_id,
    HEX_ENCODE(d.decoded_key)::VARCHAR AS decoded_key_hex
  FROM decoded AS d
)

SELECT *
FROM final
