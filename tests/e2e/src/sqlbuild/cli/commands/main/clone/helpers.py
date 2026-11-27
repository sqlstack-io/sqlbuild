"""Synthetic project and origin environment shared by clone preparation E2E tests."""

from __future__ import annotations

import shutil
import subprocess
from pathlib import Path
from textwrap import dedent

from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    execute_duckdb,
    prepare_inline_project,
    run_sqb,
)
from tests.unit.src.sqlbuild.python_nodes.helpers import node_module_files

PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": dedent(
        """
        name = "clone_preparation_project"
        adapter = "duckdb"
        default_target = "dev"

        [connections.local]
        database = "clone.duckdb"

        [targets.prod]
        schema = "prod"
        loader_schema = "raw_prod"

        [targets.prod.vars]
        region = "emea"

        [targets.prod.clone]
        allow_as_clone_origin = true

        [targets.dev]
        schema = "dev"
        loader_schema = "raw_dev"

        [targets.dev.vars]
        region = "apac"

        [targets.dev.clone]
        allow_as_clone_destination = true

        [targets.review]
        schema = "review"
        loader_schema = "raw_review"
        defer_sources_to = "prod"

        [targets.review.vars]
        region = "amer"

        [targets.review.clone]
        allow_as_clone_destination = true
        """
    ).lstrip(),
    **node_module_files(
        "python/nodes/loaders/raw.py",
        dedent(
            """
        from sqlbuild.loaders import loader


        @loader(write_strategy='replace')
        def raw_payments(ctx):
            '''Test loader raw_payments.'''
            return [{"payment_id": 1, "order_id": 1, "amount_cents": 500}]
        """
        ).lstrip(),
    ),
    "sql/sources/raw.yml": dedent(
        """
        sources:
          - name: raw_payments
            description: Test source raw_payments.
            managed: true
            columns:
              - name: payment_id
                type: BIGINT
              - name: order_id
                type: BIGINT
              - name: amount_cents
                type: BIGINT
          - name: raw_orders
            description: Test source raw_orders.
            expression: |
              SELECT *
              FROM (VALUES (1, 101, 100), (2, 102, 200))
                AS raw_orders(order_id, customer_id, amount_cents)
        """
    ).lstrip(),
    "sql/seeds/customers.csv": "customer_id,customer_name\n101,Ada\n102,Grace\n",
    "sql/seeds/schema.yml": dedent(
        """
        seeds:
          - name: customers
            description: Test seed customers.
            columns:
              - name: customer_id
                type: INTEGER
              - name: customer_name
                type: VARCHAR
        """
    ).lstrip(),
    "sql/models/staging/stg_orders.sql": dedent(
        """
        MODEL (description "Test model stg_orders.", materialized view);

        SELECT order_id, customer_id, amount_cents, '@@region' AS region FROM __source("raw_orders")
        """
    ).lstrip(),
    "sql/models/staging/stg_payments.sql": dedent(
        """
        MODEL (description "Test model stg_payments.", materialized view);

        SELECT * FROM __source("raw_payments")
        """
    ).lstrip(),
    "sql/models/marts/fct_orders.sql": dedent(
        """
        MODEL (description "Test model fct_orders.", materialized table);

        SELECT o.order_id, o.amount_cents, c.customer_name
        FROM __ref("stg_orders") AS o
        JOIN __seed("customers") AS c ON c.customer_id = o.customer_id
        """
    ).lstrip(),
    "sql/models/marts/fct_order_events.sql": dedent(
        """
        MODEL (
          description "Test model fct_order_events.",
          materialized incremental,
          incremental_strategy append
        );

        SELECT order_id, amount_cents FROM __ref("fct_orders")
        """
    ).lstrip(),
    "sql/models/marts/orders_summary.sql": dedent(
        """
        MODEL (description "Test model orders_summary.", materialized view);

        SELECT f.*, p.amount_cents AS paid_cents
        FROM __ref("fct_orders") AS f
        LEFT JOIN __ref("stg_payments") AS p ON p.order_id = f.order_id
        """
    ).lstrip(),
    "sql/models/marts/orders_backlog.sql": dedent(
        """
        MODEL (description "Test model orders_backlog.", materialized table);

        SELECT order_id FROM __ref("fct_orders") WHERE amount_cents > 150
        """
    ).lstrip(),
    "sql/models/marts/retired_orders.sql": dedent(
        """
        MODEL (description "Test model retired_orders.", materialized table, enabled false);

        SELECT order_id FROM __ref("fct_orders")
        """
    ).lstrip(),
    "sql/functions/sql/cents_to_units.sql": dedent(
        """
        FUNCTION (
          description "Test function cents_to_units.",
          arguments (amount INTEGER),
          returns DOUBLE
        );

        amount / 100.0
        """
    ).lstrip(),
}


def prepare_origin_project(tmp_path: Path) -> Path:
    """The project with its prod target loaded and built, except `orders_backlog`."""

    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="clone_preparation_project", repo_files=PROJECT_FILES
    )
    for command in (
        ("--no-color", "load", "--target", "prod"),
        ("--no-color", "build", "--target", "prod", "--exclude", "orders_backlog"),
    ):
        result: subprocess.CompletedProcess[str] = run_sqb(command=command, project_dir=project_dir)
        assert result.returncode == 0, result.stdout + result.stderr
    return project_dir


def copy_project(*, origin_project: Path, tmp_path: Path) -> Path:
    """A private copy of the built origin project for one test."""

    project_dir: Path = tmp_path / origin_project.name
    shutil.copytree(origin_project, project_dir)
    return project_dir


MISSING_UPSTREAMS: tuple[str, ...] = (
    "customers",
    "orders",
    "payments",
    "products",
    "returns",
    "shipments",
)


def plan_missing_upstreams(
    *, tmp_path: Path, default_target: str, destination_allowed: bool, target_args: tuple[str, ...]
) -> tuple[Path, tuple[str, ...], subprocess.CompletedProcess[str]]:
    """Build `MISSING_UPSTREAMS` in `prod`, then plan `report`, which reads them, in empty `dev`."""

    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="missing_upstream_project",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "missing_upstream_project"\nadapter = "duckdb"\n'
                f'default_target = "{default_target}"\n\n'
                '[connection]\ndatabase = "shop.duckdb"\n\n'
                '[targets.prod]\nschema = "prod"\n\n'
                "[targets.prod.clone]\nallow_as_clone_origin = true\n\n"
                '[targets.dev]\nschema = "dev"\n\n'
                "[targets.dev.clone]\n"
                f"allow_as_clone_destination = {str(destination_allowed).lower()}\n"
            ),
            **{
                f"sql/models/{name}.sql": (
                    f"MODEL (description '{name}.', materialized table);\n\nSELECT 1 AS id\n"
                )
                for name in MISSING_UPSTREAMS
            },
            "sql/models/report.sql": (
                "MODEL (description 'Report.', materialized table);\n\n"
                'SELECT customers.id FROM __ref("customers") AS customers\n'
                + "".join(
                    f'JOIN __ref("{name}") AS {name} USING (id)\n' for name in MISSING_UPSTREAMS[1:]
                )
            ),
        },
    )
    built: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--target", "prod"), project_dir=project_dir
    )
    assert built.returncode == 0, built.stdout + built.stderr
    plan_command: tuple[str, ...] = ("--no-color", "plan", "--select", "report", *target_args)
    return (
        project_dir,
        plan_command,
        run_sqb(command=plan_command, project_dir=project_dir),
    )


def defer_clone_with_missing_origin(
    *, tmp_path: Path, dropped_origin_table: str, before_clone: tuple[str, ...] = ()
) -> tuple[subprocess.CompletedProcess[str], subprocess.CompletedProcess[str]]:
    """Defer-clone `report` with one of its boundaries dropped in `prod`, then `plan --changed`."""

    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="partial_defer_clone_project",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "partial_defer_clone_project"\nadapter = "duckdb"\n'
                'default_target = "dev"\n\n[connection]\ndatabase = "shop.duckdb"\n\n'
                '[targets.prod]\nschema = "prod"\n\n'
                "[targets.prod.clone]\nallow_as_clone_origin = true\n\n"
                '[targets.dev]\nschema = "dev"\n\n'
                "[targets.dev.clone]\nallow_as_clone_destination = true\n"
            ),
            "sql/models/orders.sql": (
                "MODEL (description 'Orders.', materialized table);\n\nSELECT 1 AS id\n"
            ),
            "sql/models/customers.sql": (
                "MODEL (description 'Customers.', materialized table);\n\nSELECT 1 AS id\n"
            ),
            "sql/models/report.sql": (
                "MODEL (description 'Report.', materialized table);\n\n"
                'SELECT o.id FROM __ref("orders") AS o JOIN __ref("customers") AS c USING (id)\n'
            ),
        },
    )
    for target in ("dev", "prod"):
        built: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "build", "--target", target), project_dir=project_dir
        )
        assert built.returncode == 0, built.stdout + built.stderr
    for sql in (f"DROP TABLE prod.{dropped_origin_table}", *before_clone):
        execute_duckdb(db_path=project_dir / "shop.duckdb", sql=sql)
    cloned: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--select", "report", "--defer-clone-from", "prod"),
        project_dir=project_dir,
    )
    planned: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "plan", "--changed", "--select", "orders", "customers"),
        project_dir=project_dir,
    )
    return cloned, planned
