MODEL (
  description """
    One row per reconciliation check with a positive ratio, read back from an object built over
    an array holding the FLOAT ratio
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    identity_kind (type VARCHAR),
  ),
);

WITH final AS (
  SELECT
    a.check_id,
    f.value:kind::VARCHAR AS identity_kind
  FROM __ref("stg_edge_amount_checks") AS a,
    LATERAL FLATTEN(INPUT => ARRAY_CONSTRUCT_COMPACT(
      IFF(
        a.ratio > 0,
        OBJECT_CONSTRUCT('kind', 'ratio', 'identity', ARRAY_CONSTRUCT(a.check_id, a.ratio)),
        NULL
      )
    )) AS f
)

SELECT *
FROM final
