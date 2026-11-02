MODEL (
  description """
    One row per basket with the last running peak of its line numbers, from a JavaScript
    function that a pre-hook creates and SQLBuild does not declare
  """,
  tags [edge_case],
  pre_hooks [sql("create_edge_running_peaks")],
  columns (
    basket_id (type VARCHAR, nullable false, audits [not_null, unique]),
    peak_count (type NUMBER(38, 0)),
    last_peak (type NUMBER(38, 0)),
  ),
);

WITH peaks AS (
  SELECT
    l.basket_id,
    edge_running_peaks(ARRAY_AGG(l.line_no) WITHIN GROUP (ORDER BY l.line_no)) AS peaks
  FROM __ref("stg_edge_basket_lines") AS l
  GROUP BY
    l.basket_id
),

final AS (
  SELECT
    p.basket_id,
    ARRAY_SIZE(p.peaks)::NUMBER(38, 0) AS peak_count,
    p.peaks[ARRAY_SIZE(p.peaks) - 1]::NUMBER(38, 0) AS last_peak
  FROM peaks AS p
)

SELECT *
FROM final
