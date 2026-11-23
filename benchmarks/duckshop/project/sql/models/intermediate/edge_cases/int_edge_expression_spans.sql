MODEL (
  description """
    One row per edge-case price window loaded in the cursor window, with its price converted by a
    typed SQL macro and rounded by a Python macro, the reporting currency from a variable and a
    non-ASCII label, so the compiled SQL differs from the authored text before every output column;
    the acceptance case for column expression ranges that point into the compiled SQL.
  """,
  tags [edge_case],
  materialized incremental,
  incremental_strategy delete_insert,
  cursor loaded_at,
  cursor_type timestamp,
  cursor_grain second,
  cursor_inputs (
    edge__price_windows _loaded_at,
  ),
  columns (
    window_id (type VARCHAR, nullable false, audits [not_null, unique]),
    price_amount (type NUMBER(38, 6)),
    rounded_price (type NUMBER(18, 2)),
    currency (type VARCHAR),
    label (type VARCHAR),
    loaded_at (type TIMESTAMP_NTZ, nullable false),
  ),
);

SELECT
  w.window_id,
  @sql.money_from_minor(w.price_minor, 2) AS price_amount,
  @py.round_half_even("w.price_minor / 100", 2)::NUMBER(18, 2) AS rounded_price,
  '@@reporting_currency'::VARCHAR AS currency,
  'Prix à jour €'::VARCHAR AS label,
  w._loaded_at AS loaded_at
FROM __source("edge__price_windows") AS w
WHERE
  w._loaded_at >= __cursor_start()
  AND w._loaded_at < __cursor_end()
