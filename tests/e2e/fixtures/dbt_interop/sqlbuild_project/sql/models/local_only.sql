MODEL (
  description "Test model local_only.",
  tags [sqb_only],
  columns (order_id (audits [not_null])),
);

select 10 as order_id
