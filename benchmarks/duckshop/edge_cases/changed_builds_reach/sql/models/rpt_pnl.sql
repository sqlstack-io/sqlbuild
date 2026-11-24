MODEL (description "A report on revenue, refreshed by the finance job.", materialized table);
SELECT sum(revenue) AS total FROM __ref("fct_revenue")
