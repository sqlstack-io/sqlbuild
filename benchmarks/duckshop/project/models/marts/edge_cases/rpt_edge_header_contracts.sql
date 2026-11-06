MODEL (
  description "Order contract metadata retained beside a templated destination alias",
  tags [edge_case],
  alias "rpt_edge_header_contracts_${reporting_currency}",
  contract enforced,
  columns (
    order_id (type INTEGER, nullable false, audits [not_null, unique]),
    status (type order_status, nullable false, audits [not_null]),
    amount (type DECIMAL(10, 2)),
  ),
);

WITH final AS (
  SELECT
    7::INTEGER AS order_id,
    @enum("order_status").PENDING::VARCHAR AS status,
    12.50::DECIMAL(10, 2) AS amount
  FROM __seed("currencies")
  WHERE currency_code = 'USD'
)

SELECT order_id, status, amount
FROM final
