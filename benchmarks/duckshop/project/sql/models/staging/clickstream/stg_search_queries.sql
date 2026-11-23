MODEL (
  description "One row per site search query with its applied filters",
  columns (
    query_id (type VARCHAR, nullable false, audits [not_null, unique]),
    anonymous_id (type VARCHAR),
    query_text (type VARCHAR),
    query_token_count (type INT),
    results_count (type INT),
    searched_at (type TIMESTAMP_TZ, audits [not_null]),
    filter_category (type VARCHAR),
    filter_max_price_minor (type INT),
    sort_order (type VARCHAR),
    filters (type OBJECT),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("clickstream__search_queries")
),

latest AS (
  SELECT
    q.query_id,
    q.anonymous_id,
    q.query_text,
    q.results_count,
    q.searched_at,
    q.filters
  FROM src AS q
  QUALIFY ROW_NUMBER() OVER (PARTITION BY q.query_id ORDER BY q._loaded_at DESC) = 1
),

final AS (
  SELECT
    l.query_id,
    LOWER(l.anonymous_id)::VARCHAR AS anonymous_id,
    LOWER(TRIM(REGEXP_REPLACE(l.query_text, '[^\\w\\s-]', ' ')))::VARCHAR AS query_text,
    REGEXP_COUNT(TRIM(l.query_text), '\\s+')::INT + 1 AS query_token_count,
    l.results_count::INT AS results_count,
    l.searched_at,
    LOWER(l.filters:category::VARCHAR)::VARCHAR AS filter_category,
    l.filters:price.max_minor::INT AS filter_max_price_minor,
    COALESCE(l.filters:sort::VARCHAR, 'relevance')::VARCHAR AS sort_order,
    COALESCE(l.filters, OBJECT_CONSTRUCT())::OBJECT AS filters
  FROM latest AS l
)

SELECT *
FROM final
