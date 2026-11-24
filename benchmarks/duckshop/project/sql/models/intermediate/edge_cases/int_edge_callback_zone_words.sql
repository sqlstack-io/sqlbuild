MODEL (
  description """
    One row per gateway callback with the words its zone shares with a fixed list, and its zoned
    time kept in an object that keeps NULL values
  """,
  tags [edge_case],
  columns (
    callback_id (type VARCHAR, nullable false, audits [not_null, unique]),
    shared_word_count (type NUMBER(38, 0)),
    event_json (type VARCHAR),
  ),
);

WITH final AS (
  SELECT
    g.callback_id,
    ARRAY_SIZE(
      ARRAY_INTERSECTION(SPLIT(UPPER(REPLACE(g.merchant_zone, '/', ' ')), ' '), ['EUROPE', 'ASIA'])
    )::NUMBER(38, 0) AS shared_word_count,
    TO_JSON(OBJECT_CONSTRUCT_KEEP_NULL('at', g.event_at, 'zone', g.merchant_zone)) AS event_json
  FROM __ref("stg_edge_gateway_callbacks") AS g
)

SELECT *
FROM final
