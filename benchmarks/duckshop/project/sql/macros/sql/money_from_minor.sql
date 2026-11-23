MACRO (
  description "Converts an integer amount in minor units to major units for a currency exponent",
  arguments (amount NUMBER(38, 0), exponent NUMBER(38, 0), places INT CONST DEFAULT 2),
  returns NUMBER(38, 6),
);

ROUND(@sql.safe_divide(@amount, POWER(10, @exponent)::NUMBER(38, 0)), @places)::NUMBER(38, 6)
