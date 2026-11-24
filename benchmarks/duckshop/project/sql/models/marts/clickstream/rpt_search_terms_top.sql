MODEL (
  description """
    One row per search term among the 500 most searched, with zero-result and view-after-search
    rates
  """,
  constants (
    _top_terms 500,
  ),
  columns (
    search_term (type VARCHAR, nullable false, audits [not_null, unique]),
    search_rank (type INT),
    search_count (type INT),
    searching_visitors (type INT),
    zero_result_rate (type NUMBER(9, 4)),
    view_after_search_rate (type NUMBER(9, 4)),
    needs_merchandising (type BOOLEAN),
  ),
);

WITH searches AS (
  SELECT *
  FROM __ref("int_search_conversion")
),

final AS (
  SELECT
    s.search_term,
    ROW_NUMBER() OVER (ORDER BY s.search_count DESC, s.search_term)::INT AS search_rank,
    s.search_count,
    s.searching_visitors,
    s.zero_result_rate,
    s.view_after_search_rate,
    (
      s.zero_result_rate > 0.2
      OR s.view_after_search_rate < 0.1
    )::BOOLEAN AS needs_merchandising
  FROM searches AS s
  QUALIFY ROW_NUMBER() OVER (ORDER BY s.search_count DESC, s.search_term) <= @const("_top_terms")
)

SELECT *
FROM final
