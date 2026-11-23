from sqlbuild.functions import udf


@udf(
    arguments={"digits": "VARCHAR"},
    returns="BOOLEAN",
    runtime_version="3.11",
)
def main(digits: str | None) -> bool | None:
    """Whether a string of digits passes the Luhn checksum; NULL for empty or non-numeric input."""
    if not digits or not digits.isdigit():
        return None
    total = 0
    for index, char in enumerate(reversed(digits)):
        value = int(char)
        if index % 2 == 1:
            value *= 2
            if value > 9:
                value -= 9
        total += value
    return total % 10 == 0
