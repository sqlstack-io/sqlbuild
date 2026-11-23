MACRO (
  description "Division that returns NULL instead of failing on a zero denominator",
  arguments (numerator NUMBER(38, 6), denominator NUMBER(38, 6)),
  returns NUMBER(38, 6),
);

(
  @numerator / NULLIF(@denominator, 0)
)::NUMBER(38, 6)
