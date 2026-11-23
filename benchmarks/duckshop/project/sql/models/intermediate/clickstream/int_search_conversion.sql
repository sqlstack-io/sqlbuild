MODEL (
  description """
    One row per normalised search term with search volume across the site search log and the web
    tracker, zero-result rate, and how often a product view followed within ten minutes.
  """,
  columns (
    search_term (type VARCHAR, nullable false, audits [not_null, unique]),
    search_count (type INT),
    searching_visitors (type INT),
    zero_result_rate (type NUMBER(9, 4)),
    view_after_search_rate (type NUMBER(9, 4)),
    avg_results (type NUMBER(12, 2)),
    top_filter_category (type VARCHAR),
    first_searched_at_utc (type TIMESTAMP_NTZ),
  ),
);

WITH search_log AS (
  SELECT *
  FROM __ref("stg_search_queries")
),

web_searches AS (
  SELECT *
  FROM __ref("stg_web_searches")
),

product_views AS (
  SELECT *
  FROM __ref("stg_web_product_views")
),

all_searches AS (
  SELECT
    q.query_id::VARCHAR AS search_key,
    q.anonymous_id::VARCHAR AS anonymous_id,
    q.query_text::VARCHAR AS search_term,
    q.results_count::INT AS result_count,
    CONVERT_TIMEZONE('UTC', q.searched_at)::TIMESTAMP_NTZ AS searched_at_utc,
    q.filter_category::VARCHAR AS filter_category
  FROM search_log AS q
  UNION ALL
  SELECT
    w.event_id::VARCHAR AS search_key,
    w.anonymous_id::VARCHAR AS anonymous_id,
    w.search_term::VARCHAR AS search_term,
    w.result_count::INT AS result_count,
    w.searched_at_utc::TIMESTAMP_NTZ AS searched_at_utc,
    NULL::VARCHAR AS filter_category
  FROM web_searches AS w
),

search_windows AS (
  SELECT
    s.search_key,
    s.anonymous_id,
    s.searched_at_utc,
    DATEADD('minute', 10, s.searched_at_utc) AS window_end_utc
  FROM all_searches AS s
),

followed AS (
  SELECT
    w.search_key,
    COUNT(v.product_view_key) > 0 AS has_view
  FROM search_windows AS w
  LEFT JOIN product_views AS v
    ON w.anonymous_id = v.anonymous_id
    AND v.viewed_minute_utc BETWEEN w.searched_at_utc AND w.window_end_utc
  GROUP BY
    w.search_key
),

by_term AS (
  SELECT
    s.search_term,
    COUNT(*) AS search_count,
    COUNT(DISTINCT s.anonymous_id) AS searching_visitors,
    COUNT_IF(COALESCE(s.result_count, 0) = 0) AS zero_result_count,
    COUNT_IF(f.has_view) AS viewed_count,
    AVG(s.result_count) AS avg_results,
    MODE(s.filter_category) AS top_filter_category,
    MIN(s.searched_at_utc) AS first_searched_at_utc
  FROM all_searches AS s
  INNER JOIN followed AS f
    ON s.search_key = f.search_key
  WHERE
    s.search_term IS NOT NULL
    AND s.search_term <> ''
  GROUP BY
    s.search_term
),

final AS (
  SELECT
    b.search_term,
    b.search_count::INT AS search_count,
    b.searching_visitors::INT AS searching_visitors,
    ROUND(b.zero_result_count / b.search_count, 4)::NUMBER(9, 4) AS zero_result_rate,
    ROUND(b.viewed_count / b.search_count, 4)::NUMBER(9, 4) AS view_after_search_rate,
    ROUND(b.avg_results, 2)::NUMBER(12, 2) AS avg_results,
    b.top_filter_category::VARCHAR AS top_filter_category,
    b.first_searched_at_utc
  FROM by_term AS b
)

SELECT *
FROM final
