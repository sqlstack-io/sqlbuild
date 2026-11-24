MODEL (
  description """
    One row per provider settlement file whose JSON body parses, with its summary totals
  """,
  columns (
    report_id (type VARCHAR, nullable false, audits [not_null, unique]),
    provider_code (type VARCHAR, audits [not_null]),
    report_date (type DATE, audits [not_null]),
    currency_code (type VARCHAR),
    gross_minor (type INT),
    fees_minor (type INT),
    line_count (type INT),
    report (type VARIANT),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("finance__settlement_reports")
),

parsed AS (
  SELECT
    r.report_id,
    r.provider_code,
    r.report_date,
    TRY_PARSE_JSON(r.report_body) AS report
  FROM src AS r
  QUALIFY ROW_NUMBER() OVER (PARTITION BY r.report_id ORDER BY r._loaded_at DESC) = 1
),

final AS (
  SELECT
    p.report_id,
    UPPER(p.provider_code)::VARCHAR AS provider_code,
    p.report_date,
    UPPER(p.report:summary.currency::VARCHAR)::VARCHAR AS currency_code,
    p.report:summary.gross_minor::INT AS gross_minor,
    p.report:summary.fees_minor::INT AS fees_minor,
    ARRAY_SIZE(p.report:lines)::INT AS line_count,
    p.report
  FROM parsed AS p
  WHERE
    p.report IS NOT NULL
    AND IS_OBJECT(p.report)
)

SELECT *
FROM final
