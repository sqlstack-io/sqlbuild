SCHEMA (
  name money,
  description "An amount in its own currency's major units with its USD value",
  columns (
    currency_code (type VARCHAR, audits [not_null, valid_currency]),
    amount (type NUMBER(38, 6)),
    amount_usd (type NUMBER(18, 2), audits [non_negative]),
  ),
);
