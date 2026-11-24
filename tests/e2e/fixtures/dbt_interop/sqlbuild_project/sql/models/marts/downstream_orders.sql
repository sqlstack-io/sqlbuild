MODEL (
  description "Test model downstream_orders.",
  tags [nightly, finance],
  columns (order_id (audits [not_null])),
);

select order_id from __dbt_ref("analytics", "fact_orders")
