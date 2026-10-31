MODEL (
  description 'A scheduled model whose implicit scalar-function writer must be claimed.',
  tags [edge_case],
  materialized table,
  columns (
    quantity (type INTEGER, nullable false, audits [not_null]),
  ),
);

WITH final AS (
  SELECT __udf("edge_next_quantity")(q.quantity) AS quantity
  FROM __seed("edge_quantity_steps") AS q
)

SELECT *
FROM final
