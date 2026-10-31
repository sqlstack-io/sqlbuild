MODEL (
  description 'A scheduled model whose implicit scalar-function writer must be claimed.',
  tags [edge_case],
  materialized table,
);

SELECT __udf("edge_next_quantity")(CAST(23 AS INTEGER)) AS quantity
