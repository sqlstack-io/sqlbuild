MACRO (
  description """
    Major-unit amount and its USD value from a minor-unit amount, currency exponent and USD rate
  """,
  arguments (
    src RELATION (
      amount_minor NUMBER(38, 0),
      minor_unit_exponent NUMBER(38, 0),
      rate_to_usd NUMBER(18, 8),
    ),
  ),
);

SELECT
  @sql.money_from_minor(@src.amount_minor, @src.minor_unit_exponent, places := 6) AS amount,
  ROUND(
    @sql.money_from_minor(@src.amount_minor, @src.minor_unit_exponent, places := 6) * @src.rate_to_usd,
    2
  )::NUMBER(18, 2) AS amount_usd
