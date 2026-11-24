MODEL (
  description "One row per visitor, product and minute of product detail viewing on web",
  columns (
    product_view_key (type VARCHAR, nullable false, audits [not_null, unique]),
    anonymous_id (type VARCHAR, audits [not_null]),
    customer_id (type VARCHAR),
    sku (type VARCHAR, audits [not_null]),
    viewed_minute_utc (type TIMESTAMP_NTZ),
    list_name (type VARCHAR),
    list_position (type INT),
  ),
);

WITH web_events AS (
  SELECT *
  FROM __ref("stg_web_events")
),

views AS (
  SELECT
    e.event_id,
    e.anonymous_id,
    e.customer_id,
    @sql.normalise_sku(e.properties:sku::VARCHAR) AS sku,
    DATE_TRUNC('minute', e.event_at_utc) AS viewed_minute_utc,
    e.properties:list.name::VARCHAR AS list_name,
    e.properties:list.position::INT AS list_position
  FROM web_events AS e
  WHERE
    e.event_type = 'product_view'
    AND NOT e.is_bot
),

final AS (
  SELECT
    (
      v.anonymous_id || '|' || v.sku || '|' || TO_VARCHAR(v.viewed_minute_utc, 'YYYYMMDDHH24MI')
    )::VARCHAR AS product_view_key,
    v.anonymous_id,
    v.customer_id,
    v.sku,
    v.viewed_minute_utc,
    v.list_name::VARCHAR AS list_name,
    v.list_position
  FROM views AS v
  WHERE
    v.sku IS NOT NULL
    AND v.anonymous_id IS NOT NULL
  QUALIFY
    ROW_NUMBER() OVER (
      PARTITION BY v.anonymous_id, v.sku, v.viewed_minute_utc
      ORDER BY v.event_id
    ) = 1
)

SELECT *
FROM final
