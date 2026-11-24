MACRO (
  description "Digits-only phone number with a leading plus, or NULL when too short",
  arguments (phone VARCHAR),
  returns VARCHAR,
);

IFF(LENGTH(REGEXP_REPLACE(@phone, '[^0-9]', '')) >= 7, '+'
|| LTRIM(
  REGEXP_REPLACE(@phone, '[^0-9]', ''),
  '0'
), NULL)
