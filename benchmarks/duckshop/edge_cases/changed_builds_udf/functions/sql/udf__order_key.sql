FUNCTION (
  description "Display key of an order, ten times its number",
  arguments (p_order_id INTEGER),
  returns INTEGER,
);

p_order_id * 10
