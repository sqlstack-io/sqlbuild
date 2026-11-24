MACRO (
  description "Lower-cased, trimmed email address, or NULL when it is not a plausible address",
  arguments (email VARCHAR),
  returns VARCHAR,
);

IFF(
  REGEXP_LIKE(LOWER(TRIM(@email)), @const("email_pattern")),
  REGEXP_REPLACE(LOWER(TRIM(@email)), '\\+[^@]*@', '@'),
  NULL
)
