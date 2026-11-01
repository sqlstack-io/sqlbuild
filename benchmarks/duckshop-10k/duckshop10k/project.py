"""Assemble the whole DuckShop 10K project in memory, deterministically."""

from __future__ import annotations

from dataclasses import dataclass

from duckshop10k import GENERATOR_VERSION, PROJECT_NAME, SEED
from duckshop10k.core import Project
from duckshop10k.declarations import currencies_seed, declare_all
from duckshop10k.extras import build_extras
from duckshop10k.layers import build_intermediate
from duckshop10k.mart_layer import build_marts
from duckshop10k.rules_gen import build_rules
from duckshop10k.staging import StagedTable, stage_collector, stage_instance
from duckshop10k.tests_gen import build_scenarios, build_tests
from duckshop10k.world import Instance, instances, storefront_instances


@dataclass(frozen=True)
class Profile:
    """How many instances of each system to generate. `full` is the published project."""

    name: str
    per_system: int | None


PROFILES: dict[str, Profile] = {
    "full": Profile("full", None),
    "smoke": Profile("smoke", 2),
}


@dataclass
class Staging:
    tables: dict[str, dict[str, StagedTable]]
    events: dict[str, tuple[str, list[str]]]

    def table(self, instance: Instance, table: str) -> StagedTable:
        return self.tables[instance.code][table]


def _limit(values: tuple[Instance, ...], profile: Profile) -> tuple[Instance, ...]:
    return values if profile.per_system is None else values[: profile.per_system]


def selected_instances(profile: Profile) -> dict[str, tuple[Instance, ...]]:
    systems = {"storefront": storefront_instances(), **instances()}
    return {system: _limit(values, profile) for system, values in systems.items()}


_STAGING_FOLDER = {
    "storefront": "storefronts",
    "marketplace": "marketplaces",
    "carrier": "carriers",
    "warehouse": "warehouses",
    "store": "stores",
    "supplier": "suppliers",
    "gateway": "payments",
    "ad_network": "advertising",
    "messaging": "messaging",
    "affiliate": "affiliates",
    "collector": "clickstream",
    "support": "support",
    "ledger": "ledgers",
    "wholesale": "wholesale",
    "returns": "returns",
    "catalogue": "catalogue",
}


def build(profile_name: str = "full") -> dict[str, str]:
    profile = PROFILES[profile_name]
    project = Project()
    declare_all(project)
    systems = selected_instances(profile)
    staging = Staging(tables={}, events={})
    for system, values in systems.items():
        for instance in values:
            folder = f"models/staging/{_STAGING_FOLDER[system]}/{instance.code}"
            if system == "collector":
                staging.events[instance.code] = stage_collector(project, instance, folder)
            else:
                staging.tables[instance.code] = {
                    table.archetype.table: table
                    for table in stage_instance(project, instance, folder)
                }
    intermediate = build_intermediate(project, staging.tables, staging.events, systems)
    build_marts(project, intermediate, systems)
    build_extras(project, intermediate)
    build_tests(project, staging.tables, intermediate)
    build_scenarios(project, staging.tables)
    build_rules(project)
    _static_files(project)
    project.write("seeds/source_instances.csv", instances_seed(systems))
    return project.render()


def _static_files(project: Project) -> None:
    project.write("sqlbuild_project.toml", project_toml())
    project.write(".gitignore", "sqlbuild_local.toml\ntarget/\nlogs/\n.sqlbuild/\n")
    project.write("seeds/currencies.csv", currencies_seed())
    project.write(
        "seeds/seeds.yml",
        "seeds:\n"
        "  - name: currencies\n"
        "    description: ISO currencies with the number of decimal places in their minor unit\n"
        "    columns:\n"
        "      - name: currency_code\n"
        "        type: VARCHAR\n"
        "        audits:\n"
        "          - not_null\n"
        "          - unique\n"
        "      - name: minor_unit_exponent\n"
        "        type: INT\n"
        "  - name: source_instances\n"
        "    description: Every deployed source system with its region, time zone and currency\n"
        "    columns:\n"
        "      - name: instance_code\n"
        "        type: VARCHAR\n"
        "        audits:\n"
        "          - not_null\n"
        "          - unique\n"
        "      - name: system\n"
        "        type: VARCHAR\n"
        "      - name: region_code\n"
        "        type: VARCHAR\n"
        "      - name: timezone\n"
        "        type: VARCHAR\n"
        "      - name: currency_code\n"
        "        type: VARCHAR\n"
        "        audits:\n"
        "          - relationships:\n"
        '              to: __seed("currencies")\n'
        "              field: currency_code\n",
    )
    project.write("README.md", readme())


def instances_seed(systems: dict[str, tuple[Instance, ...]]) -> str:
    rows = ["instance_code,system,region_code,timezone,currency_code"]
    for system, values in systems.items():
        rows.extend(
            f"{i.code.upper()},{system},{i.region},{i.timezone},{i.currency}" for i in values
        )
    return "\n".join(rows) + "\n"


def project_toml() -> str:
    return f'''# Generated by DuckShop 10K generator version {GENERATOR_VERSION} (seed {SEED}).
# Do not edit; change the generator in the sqlbuild repository instead.
name = "{PROJECT_NAME}"
adapter = "snowflake"
default_target = "dev"

# Placeholders only. Compiling needs no connection.
[connections.warehouse]
account = "example-account"
user = "example_user"
role = "EXAMPLE_ROLE"
warehouse = "EXAMPLE_WH"

[targets.dev]
connection = "warehouse"
database = "DUCKSHOP_10K"
schema = "DEV"

[settings]
default_audit_severity = "error"
unit_test_max_real_models = 6

[defaults]
materialized = "table"

[path_defaults.staging]
materialized = "view"
tags = ["staging"]

[path_defaults.intermediate]
materialized = "table"
tags = ["intermediate"]

[path_defaults.marts]
materialized = "table"
tags = ["marts"]

[path_defaults.snapshots]
tags = ["snapshots"]

[vars]
reporting_currency = "USD"
valuation_date = "2026-03-31"
lookup_customer_id = "C000001"

[rules]
select = ["SQLCLEAN", "SQBRGRAPH", "SQBRCONTRACT", "SQBRTEST", "XSQBRDUCK"]
# SQLCLEAN021: `SELECT * FROM final` and the change-log macro's pass-through CTEs are the house
#   style; each model's columns block, not its terminal SELECT, owns the output shape.
# SQBRCONTRACT101: exact contracts are reserved for published dim_ and fct_ models.
# SQBRTEST202: a sample of models carries unit tests, as in a project of this size.
ignore = ["SQLCLEAN021", "SQBRCONTRACT101", "SQBRTEST202"]
'''


def readme() -> str:
    return f"""# DuckShop 10K

Generated by the DuckShop 10K generator, version {GENERATOR_VERSION}, seed {SEED}. Every file is
generated: change the generator in the sqlbuild repository (`benchmarks/duckshop-10k/`) and
regenerate instead of editing this project.

Compile it with no warehouse connection:

```bash
sqb compile
```
"""
