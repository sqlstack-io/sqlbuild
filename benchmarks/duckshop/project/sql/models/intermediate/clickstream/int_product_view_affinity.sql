MODEL (
  description """
    One row per ordered pair of SKUs viewed by the same visitor within an hour, with the number of
    visitors who viewed both and the lift over chance.
  """,
  columns (
    sku_pair_key (type VARCHAR, nullable false, audits [not_null, unique]),
    sku (type VARCHAR, audits [not_null]),
    co_viewed_sku (type VARCHAR, audits [not_null]),
    co_viewers (type INT),
    sku_viewers (type INT),
    co_view_rate (type NUMBER(9, 6)),
    lift (type NUMBER(12, 4)),
    pair_rank (type INT),
  ),
);

WITH product_views AS (
  SELECT *
  FROM __ref("stg_web_product_views")
),

visitor_views AS (
  SELECT
    v.anonymous_id,
    v.sku,
    v.viewed_minute_utc,
    DATEADD('hour', 1, v.viewed_minute_utc) AS window_end_utc
  FROM product_views AS v
),

pairs AS (
  SELECT DISTINCT
    a.anonymous_id,
    a.sku,
    b.sku AS co_viewed_sku
  FROM visitor_views AS a
  INNER JOIN visitor_views AS b
    ON a.anonymous_id = b.anonymous_id
    AND a.sku <> b.sku
    AND b.viewed_minute_utc BETWEEN a.viewed_minute_utc AND a.window_end_utc
),

pair_counts AS (
  SELECT
    p.sku,
    p.co_viewed_sku,
    COUNT(DISTINCT p.anonymous_id) AS co_viewers
  FROM pairs AS p
  GROUP BY
    p.sku,
    p.co_viewed_sku
),

sku_counts AS (
  SELECT
    v.sku,
    COUNT(DISTINCT v.anonymous_id) AS sku_viewers
  FROM visitor_views AS v
  GROUP BY
    v.sku
),

visitor_total AS (
  SELECT COUNT(DISTINCT v.anonymous_id) AS total_viewers
  FROM visitor_views AS v
),

final AS (
  SELECT
    (
      pc.sku || '>' || pc.co_viewed_sku
    )::VARCHAR AS sku_pair_key,
    pc.sku,
    pc.co_viewed_sku,
    pc.co_viewers::INT AS co_viewers,
    a.sku_viewers::INT AS sku_viewers,
    ROUND(pc.co_viewers / NULLIF(a.sku_viewers, 0), 6)::NUMBER(9, 6) AS co_view_rate,
    ROUND(
      (
        pc.co_viewers / NULLIF(a.sku_viewers, 0)
      )
      / NULLIF(b.sku_viewers / NULLIF(t.total_viewers, 0), 0),
      4
    )::NUMBER(12, 4) AS lift,
    RANK() OVER (PARTITION BY pc.sku ORDER BY pc.co_viewers DESC)::INT AS pair_rank
  FROM pair_counts AS pc
  INNER JOIN sku_counts AS a
    ON pc.sku = a.sku
  INNER JOIN sku_counts AS b
    ON pc.co_viewed_sku = b.sku
  CROSS JOIN visitor_total AS t
)

SELECT *
FROM final
