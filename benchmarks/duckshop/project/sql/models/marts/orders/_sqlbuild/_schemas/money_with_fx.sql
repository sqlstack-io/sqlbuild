SCHEMA (
  name money_with_fx,
  extends money,
  description "A money amount with the reference rate used to convert it and that rate's date",
  columns (
    rate_to_usd (type NUMBER(18, 8), audits [not_null]),
    rate_date (type DATE),
  ),
);
