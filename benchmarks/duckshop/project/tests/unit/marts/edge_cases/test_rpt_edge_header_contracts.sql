TEST (name "rpt_edge_header_contracts__keeps_enum_contract_with_templated_alias");

WITH __seed__currencies AS (
  SELECT 'USD' AS currency_code
  UNION ALL
  SELECT 'EUR' AS currency_code
),
__expected__rpt_edge_header_contracts AS (
  SELECT
    7::INTEGER AS order_id,
    'PENDING'::VARCHAR AS status,
    12.50::DECIMAL(10, 2) AS amount
)

SELECT 1
