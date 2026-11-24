MACRO (
  description "Whether the caller's order_status column equals one order status",
  arguments (status ENUM order_status),
  returns BOOLEAN,
);

order_status = @status
