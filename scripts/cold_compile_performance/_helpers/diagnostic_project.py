"""Synthetic workloads for error-only compile paths and deep poisoned lineage."""

from pathlib import Path


def write_diagnostic_project(
    *, project_dir: Path, model_count: int = 3, width: int = 32, depth: int = 100
) -> None:
    """Generate large macro-backed CTE models with independent reference and conversion errors."""
    project_dir.mkdir(parents=True, exist_ok=True)
    models: Path = project_dir / "sql" / "models"
    models.mkdir(parents=True, exist_ok=True)
    (project_dir / "sqlbuild_project.toml").write_text(
        'name = "diagnostic_orders"\nadapter = "duckdb"\n[rules]\nselect = []\n'
    )
    columns: list[str] = [f"amount_{index:03}" for index in range(width)]
    declarations: str = ", ".join(f"{column} (type INTEGER)" for column in columns)
    (models / "orders.sql").write_text(
        "MODEL (description 'Test model orders.', "
        f"materialized view, contract enforced, columns ({declarations}));\nSELECT "
        + ", ".join(f"CAST(1 AS INTEGER) AS {column}" for column in columns)
    )
    for model_index in range(model_count):
        ctes: list[str] = ['input_orders AS (SELECT * FROM __ref("orders"))']
        previous: str = "input_orders"
        for level in range(depth):
            name: str = f"stage_{level:03}"
            projection: str = ",\n".join(f"  {column} + 1 AS {column}" for column in columns)
            ctes.append(f"{name} AS (\nSELECT\n{projection}\nFROM {previous}\n)")
            previous = name
        errors: list[str] = [f"missing_{index:03} AS broken_{index:03}" for index in range(50)]
        conversions: list[str] = [
            f"{columns[index % width]} = 'shipped' AS risky_{index:03}" for index in range(50)
        ]
        sql: str = "MODEL (description 'Test model.', materialized view);\nWITH\n" + ",\n".join(
            ctes
        )
        sql += "\nSELECT\n" + ",\n".join((*errors, *conversions)) + f"\nFROM {previous}\n"
        name = f"large_orders_{model_index}"
        (models / f"{name}.sql").write_text(sql)
        (models / f"downstream_orders_{model_index}.sql").write_text(
            "MODEL (description 'Test model.', materialized view);\nSELECT "
            + ", ".join(f"broken_{index:03} + 1 AS result_{index:03}" for index in range(50))
            + f' FROM __ref("{name}")'
        )


def write_cascade_project(*, project_dir: Path, depth: int, width: int = 50) -> None:
    """Generate a reverse-named DAG with many independently poisoned output columns."""
    project_dir.mkdir(parents=True, exist_ok=True)
    models: Path = project_dir / "sql" / "models"
    models.mkdir(parents=True, exist_ok=True)
    (project_dir / "sqlbuild_project.toml").write_text(
        'name = "cascade_orders"\nadapter = "duckdb"\n[rules]\nselect = []\n'
    )
    columns: list[str] = [f"amount_{index:03}" for index in range(width)]
    declarations: str = ", ".join(f"{column} (type INTEGER)" for column in columns)
    header: str = (
        "MODEL (description 'Test model.', "
        f"materialized view, contract enforced, columns ({declarations}));\n"
    )
    (models / "input_orders.sql").write_text(
        "MODEL (description 'Test model input_orders.', "
        f"materialized view, contract enforced, columns ({declarations}, "
        "ordered_at (type TIMESTAMP)));\nSELECT "
        + ", ".join(f"CAST(1 AS INTEGER) AS {column}" for column in columns)
        + ", CAST('2026-04-01' AS TIMESTAMP) AS ordered_at"
    )
    root: str = ", ".join(f"{column} + ordered_at AS {column}" for column in columns)
    previous: str = f"orders_{depth:03}"
    (models / f"{previous}.sql").write_text(
        header + "SELECT " + root + ' FROM __ref("input_orders")'
    )
    for level in reversed(range(depth)):
        name: str = f"orders_{level:03}"
        (models / f"{name}.sql").write_text(
            header
            + "SELECT "
            + ", ".join(f"{column} > TIMESTAMP '2026-04-01' AS {column}" for column in columns)
            + f' FROM __ref("{previous}")'
        )
        previous = name


def write_function_name_project(*, project_dir: Path, model_count: int, calls: int) -> None:
    """Generate wide Snowflake models mixing unsupported, accepted, and unknown function calls."""
    project_dir.mkdir(parents=True, exist_ok=True)
    models: Path = project_dir / "sql" / "models"
    models.mkdir(parents=True, exist_ok=True)
    (project_dir / "sqlbuild_project.toml").write_text(
        'name = "function_name_orders"\nadapter = "snowflake"\n[rules]\nselect = []\n'
    )
    for model_index in range(model_count):
        projections: list[str] = []
        for index in range(calls):
            projections.extend(
                (
                    f"  STARTS_WITH(sku, 'a{index}') AS unsupported_{index:05}",
                    f"  STARTSWITH(sku, 'a{index}') AS accepted_{index:05}",
                    f"  normalize_sku(sku, {index}) AS unknown_{index:05}",
                )
            )
        (models / f"wide_products_{model_index}.sql").write_text(
            "MODEL (description 'Test model.', database warehouse, schema analytics);\nSELECT\n"
            + ",\n".join(projections)
            + "\nFROM (SELECT 'anvil' AS sku) AS products\nWHERE NOT (sku IN ('a', 'b'))\n"
        )


def write_single_model_diagnostic_project(*, project_dir: Path, diagnostics: int) -> None:
    """Generate one wide model whose projections each raise an unknown-column or type error."""
    project_dir.mkdir(parents=True, exist_ok=True)
    models: Path = project_dir / "sql" / "models"
    models.mkdir(parents=True, exist_ok=True)
    (project_dir / "sqlbuild_project.toml").write_text(
        'name = "dense_diagnostic_orders"\nadapter = "duckdb"\n[rules]\nselect = []\n'
    )
    (models / "orders.sql").write_text(
        "MODEL (description 'Test model orders.', "
        "materialized view, contract enforced, columns (order_id (type INTEGER), "
        "status (type VARCHAR)));\nSELECT CAST(1 AS INTEGER) AS order_id, "
        "CAST('open' AS VARCHAR) AS status"
    )
    projections: list[str] = [
        f"  o.missing_{index:05} AS broken_{index:05}"
        if index % 2 == 0
        else f"  o.order_id = 'shipped_{index:05}' AS risky_{index:05}"
        for index in range(diagnostics)
    ]
    (models / "order_diagnostics.sql").write_text(
        "MODEL (description 'Test model order_diagnostics.', materialized view);\nSELECT\n"
        + ",\n".join(projections)
        + '\nFROM __ref("orders") AS o\n'
    )


def write_undescribed_project(*, project_dir: Path, model_count: int, width: int = 6) -> None:
    """Generate models, seeds, and one large source file whose resources all lack descriptions."""
    project_dir.mkdir(parents=True, exist_ok=True)
    for folder in ("models", "sources", "seeds"):
        (project_dir / "sql" / folder).mkdir(parents=True, exist_ok=True)
    (project_dir / "sqlbuild_project.toml").write_text(
        'name = "undescribed_orders"\nadapter = "duckdb"\n[rules]\nselect = []\n'
    )
    columns: list[str] = [f"amount_{index:03}" for index in range(width)]
    sources: list[str] = ["sources:"]
    seeds: list[str] = ["seeds:"]
    for index in range(model_count):
        sources.extend(
            (f"  - name: orders_{index:05}", "    schema: main", f"    table: orders_{index:05}")
        )
        sources.append("    columns:")
        sources.extend(f"      - name: {column}\n        type: INTEGER" for column in columns)
        (project_dir / "sql" / "models" / f"order_totals_{index:05}.sql").write_text(
            "MODEL (materialized view);\n"
            f'SELECT {", ".join(columns)} FROM __source("orders_{index:05}")\n'
        )
        if index % 10 == 0:
            seeds.append(f"  - name: products_{index:05}\n    columns:")
            seeds.extend(f"      - name: {column}\n        type: INTEGER" for column in columns)
            (project_dir / "sql" / "seeds" / f"products_{index:05}.csv").write_text(
                ",".join(columns) + "\n" + ",".join("1" for _ in columns) + "\n"
            )
    (project_dir / "sql" / "sources" / "orders.yml").write_text("\n".join(sources) + "\n")
    (project_dir / "sql" / "seeds" / "products.yml").write_text("\n".join(seeds) + "\n")
