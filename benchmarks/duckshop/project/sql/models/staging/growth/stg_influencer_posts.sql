MODEL (
  description "One row per live creator post with engagement metrics",
  columns (
    post_id (type VARCHAR, nullable false, audits [not_null, unique]),
    creator_handle (type VARCHAR),
    platform (type VARCHAR),
    posted_at (type TIMESTAMP_TZ),
    promo_code (type VARCHAR),
    impressions (type INT),
    likes (type INT),
    engagement_rate (type NUMBER(9, 6)),
    metrics (type OBJECT),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("growth__influencer_posts")
),

@sql.cdc_latest(src, src.key := (post_id))

final AS (
  SELECT
    p.post_id,
    LOWER(LTRIM(p.creator_handle, '@'))::VARCHAR AS creator_handle,
    LOWER(p.platform)::VARCHAR AS platform,
    p.posted_at,
    UPPER(TRIM(p.promo_code))::VARCHAR AS promo_code,
    p.metrics:impressions::INT AS impressions,
    p.metrics:likes::INT AS likes,
    ROUND(
      (
        COALESCE(p.metrics:likes::INT, 0)
          + COALESCE(p.metrics:comments::INT, 0)
          + COALESCE(p.metrics:shares::INT, 0)
      )
      / NULLIF(p.metrics:impressions::INT, 0),
      6
    )::NUMBER(9, 6) AS engagement_rate,
    p.metrics::OBJECT AS metrics,
    @sql.load_metadata(p)
  FROM cdc_latest AS p
)
SELECT *
FROM final
