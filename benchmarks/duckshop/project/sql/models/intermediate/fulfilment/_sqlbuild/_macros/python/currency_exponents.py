"""Currency exponent lookup that needs compile-time logic."""

# Minor-unit exponents per ISO currency, mirroring the currencies seed. Used where a CASE expression
# is cheaper than a join, such as inside window frames.
_EXPONENTS = {"USD": 2, "EUR": 2, "GBP": 2, "JPY": 0, "AUD": 2, "KWD": 3}


def minor_unit_exponent(currency_column: str) -> str:
    """CASE expression giving the minor-unit exponent of a currency code column (default 2)."""
    branches = " ".join(f"WHEN '{code}' THEN {exp}" for code, exp in sorted(_EXPONENTS.items()))
    return f"(CASE {currency_column} {branches} ELSE 2 END)"
