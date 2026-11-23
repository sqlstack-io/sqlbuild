FUNCTION (
  description "Product family prefix of a canonical SKU, such as DECOY for DECOY-TEAL-08",
  arguments (sku VARCHAR),
  returns VARCHAR,
);

UPPER(REGEXP_SUBSTR(sku, '^[A-Za-z]+'))
