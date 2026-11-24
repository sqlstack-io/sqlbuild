MODEL (
  description "One row per live product review with moderation outcome",
  columns (
    review_id (type VARCHAR, nullable false, audits [not_null, unique]),
    sku (type VARCHAR, audits [not_null]),
    customer_id (type VARCHAR),
    rating (type INT, audits [accepted_values (values [1, 2, 3, 4, 5])]),
    title (type VARCHAR),
    body_length (type INT),
    submitted_at (type TIMESTAMP_TZ),
    moderation_status (type VARCHAR),
    flagged_terms (type ARRAY),
    mentions_url (type BOOLEAN),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("storefront__reviews")
),

@sql.cdc_latest(src, src.key := (review_id))

final AS (
  SELECT
    r.review_id,
    @sql.normalise_sku(r.sku) AS sku,
    r.customer_id,
    LEAST(GREATEST(r.rating, 1), 5)::INT AS rating,
    TRIM(r.title)::VARCHAR AS title,
    LENGTH(TRIM(r.body))::INT AS body_length,
    r.submitted_at,
    LOWER(COALESCE(r.moderation:status::VARCHAR, 'pending'))::VARCHAR AS moderation_status,
    COALESCE(r.moderation:flags, ARRAY_CONSTRUCT())::ARRAY AS flagged_terms,
    REGEXP_LIKE(r.body, '.*https?://.*', 'is') AS mentions_url,
    @sql.load_metadata(r)
  FROM cdc_latest AS r
)
SELECT *
FROM final
