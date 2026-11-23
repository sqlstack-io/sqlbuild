MODEL (
  description """
    One row per partner payload message with its promotion codes and tags combined as arrays, and
    category eligibility checked against ids the partner sent in mixed number spellings
  """,
  tags [edge_case],
  columns (
    message_id (type VARCHAR, nullable false, audits [not_null, unique]),
    codes (type ARRAY),
    codes_compact (type ARRAY),
    tags_compact (type ARRAY),
    tags_kept (type ARRAY),
    tags_all (type ARRAY),
    tags_distinct (type ARRAY),
    category_position (type INT),
    is_eligible (type BOOLEAN),
    shares_extra_tags (type BOOLEAN),
  ),
);

WITH payloads AS (
  SELECT
    s.message_id,
    s.payload
  FROM __ref("stg_edge_partner_payloads") AS s
),

final AS (
  SELECT
    p.message_id,
    ARRAY_CONSTRUCT(
      p.payload:promo_code::VARCHAR,
      p.payload:gift_code::VARCHAR
    )::ARRAY AS codes,
    ARRAY_CONSTRUCT_COMPACT(
      p.payload:promo_code::VARCHAR,
      p.payload:gift_code::VARCHAR
    )::ARRAY AS codes_compact,
    ARRAY_COMPACT(p.payload:tags::ARRAY)::ARRAY AS tags_compact,
    -- ARRAY_EXCEPT and ARRAY_DISTINCT leave element order unspecified; sort ascending, NULLs last.
    ARRAY_SORT(
      ARRAY_EXCEPT(p.payload:tags::ARRAY, p.payload:removed_tags::ARRAY),
      TRUE,
      FALSE
    )::ARRAY AS tags_kept,
    ARRAY_CAT(
      p.payload:tags::ARRAY,
      p.payload:extra_tags::ARRAY
    )::ARRAY AS tags_all,
    ARRAY_SORT(
      ARRAY_DISTINCT(p.payload:tags::ARRAY),
      TRUE,
      FALSE
    )::ARRAY AS tags_distinct,
    ARRAY_POSITION(p.payload:category_id, p.payload:eligible_ids::ARRAY)::INT AS category_position,
    ARRAY_CONTAINS(p.payload:category_id, p.payload:eligible_ids::ARRAY)::BOOLEAN AS is_eligible,
    ARRAYS_OVERLAP(
      p.payload:tags::ARRAY,
      p.payload:extra_tags::ARRAY
    )::BOOLEAN AS shares_extra_tags
  FROM payloads AS p
)

SELECT *
FROM final
