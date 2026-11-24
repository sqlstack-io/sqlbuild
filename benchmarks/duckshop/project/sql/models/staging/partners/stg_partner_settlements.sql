MODEL (
  description "One row per live partner settlement statement with fee totals",
  columns (
    settlement_id (type VARCHAR, nullable false, audits [not_null, unique]),
    partner_code (type VARCHAR, audits [not_null]),
    period_start (type DATE),
    period_end (type DATE),
    currency_code (type VARCHAR),
    gross_sales_minor (type INT),
    commission_minor (type INT),
    other_fees_minor (type INT),
    net_payable_minor (type INT),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("partners__partner_settlements")
),

@sql.cdc_latest(src, src.key := (settlement_id))

final AS (
  SELECT
    s.settlement_id,
    UPPER(s.partner_code)::VARCHAR AS partner_code,
    s.period_start,
    s.period_end,
    UPPER(s.currency_code)::VARCHAR AS currency_code,
    s.statement:gross_sales_minor::INT AS gross_sales_minor,
    s.statement:commission_minor::INT AS commission_minor,
    COALESCE(REDUCE(
      s.statement:fees::ARRAY,
      0,
      (acc, f) -> acc + f:amount_minor::INT
    ), 0)::INT AS other_fees_minor,
    (
      s.statement:gross_sales_minor::INT - s.statement:commission_minor::INT
      - COALESCE(REDUCE(
        s.statement:fees::ARRAY,
        0,
        (acc, f) -> acc + f:amount_minor::INT
      ), 0)
    )::INT AS net_payable_minor,
    @sql.load_metadata(s)
  FROM cdc_latest AS s
)
SELECT *
FROM final
