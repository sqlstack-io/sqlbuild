MODEL (
  description """
    One row per edge-case price window, each column qualified by the source reference call itself,
    so every output column's expression starts at text the compiler rewrites; the case for column
    expression ranges that must be dropped rather than moved onto the relation name.
  """,
  tags [edge_case],
  columns (
    window_id (type VARCHAR, nullable false, audits [not_null, unique]),
    sku (type VARCHAR),
  ),
);

SELECT
  __source("edge__price_windows").window_id,
  __source("edge__price_windows").sku
FROM __source("edge__price_windows")
