MACRO (
  description "Canonical SKU: upper case with whitespace and underscores removed",
  arguments (sku VARCHAR),
  returns VARCHAR,
);

UPPER(TRANSLATE(
  REGEXP_REPLACE(TRIM(@sku), '\\s+', ''),
  '_',
  '-'
))
