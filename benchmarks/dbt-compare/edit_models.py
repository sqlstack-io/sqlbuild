"""Edit the SQL body of evenly spaced models in a generated dense project.

Usage: python edit_models.py PROJECT_DIR --models TOTAL --count N --value V

Works on both the SQLBuild and the dbt project. Each selected model's last plain
`COALESCE(b.amount, <n>)` projection gets the constant V, so the edit changes the query but not the
model's columns, types, references or header.
"""

import argparse
import re
from pathlib import Path

PATTERN = re.compile(r"CAST\(COALESCE\(b\.amount, \d+\)")


def model_name(index: int) -> str:
    return f"sales{index // 1000:03d}__int_clean__orders_{index:05d}"


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("project", type=Path)
    parser.add_argument("--models", type=int, required=True)
    parser.add_argument("--count", type=int, required=True)
    parser.add_argument("--value", type=int, required=True)
    args = parser.parse_args()
    step = args.models // args.count
    for k in range(args.count):
        index = k * step + step // 2
        models = args.project / "sql" / "models"
        if not models.is_dir():
            models = args.project / "models"
        path = (
            models
            / f"sales{index // 1000:03d}"
            / "intermediate"
            / "clean"
            / f"batch{index // 5:05d}"
            / f"{model_name(index)}.sql"
        )
        text = path.read_text()
        matches = list(PATTERN.finditer(text))
        assert matches, path
        last = matches[-1]
        replacement = f"CAST(COALESCE(b.amount, {args.value})"
        assert last.group(0) != replacement, path
        path.write_text(text[: last.start()] + replacement + text[last.end() :])


if __name__ == "__main__":
    main()
