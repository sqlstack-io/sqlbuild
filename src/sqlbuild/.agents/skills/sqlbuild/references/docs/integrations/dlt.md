<!-- generated-by: sqlbuild skills -->

# dlt

> Declarative dlt sources in YAML, or full dlt pipelines inside Python source loaders.

Online: https://sqlbuild.com/docs/integrations/dlt/

## Contents

- Install
- Declarative dlt sources (YAML)
- dlt inside a Python loader
- Build integration

[dlt](https://dlthub.com) is an open-source Python library for loading data from APIs, databases, cloud storage, and more. SQLBuild integrates with dlt two ways:

- **Declarative dlt sources** - configure dlt directly in your source YAML, no Python code. SQLBuild runs the dlt pipeline as part of the build lifecycle. Best for the common REST API, SQL database, and filesystem cases.
- **dlt inside a Python loader** - wrap any `dlt.pipeline(...)` in a SQLBuild [source loader](../concepts/python-nodes/loaders.md) for full control when you need dlt features beyond the declarative surface.

## Install

```bash
pip install 'sqlbuild[dlt]'
# or
uv pip install 'sqlbuild[dlt]'
```

This installs dlt with the `duckdb`, `filesystem`, and `sql_database` extras alongside SQLBuild.

## Declarative dlt sources (YAML)

Declare a `dlt_sources` list in a `sources/*.yml` file. Each entry has a `type`, a `config` block passed to the dlt source, and a list of `resources` that become SQLBuild managed sources. SQLBuild generates a synthetic loader per resource and writes into the database its adapter manages, no Python and no destination setup required.

Supported source types: `rest_api`, `sql_database`, `filesystem`.

### REST API

```yaml
# sources/github.yml
dlt_sources:
  - type: rest_api
    config:
      client:
        base_url: https://api.github.com/
        headers:
          Authorization: "Bearer ${github_token}"
        paginator:
          type: header_link
    resources:
      - name: raw_github_issues
        description: Issues from the GitHub API
        write_disposition: append
        endpoint:
          path: "repos/${github_owner}/${github_repo}/issues"
          params:
            state: all
            per_page: 100
```

`rest_api` requires `client.base_url` in `config`. Each resource needs an `endpoint` mapping; the dlt resource name comes from `endpoint.name` if present, otherwise the resource `name`.

### SQL database

```yaml
# sources/postgres.yml
dlt_sources:
  - type: sql_database
    config:
      credentials: "${postgres_connection_string}"
    resources:
      - name: raw_customers
        description: Customers from the CRM database
        table: customers
        write_disposition: merge
        primary_key: id
```

`sql_database` requires `credentials` in `config`, and each resource requires a `table`.

### Filesystem

```yaml
# sources/files.yml
dlt_sources:
  - type: filesystem
    config:
      bucket_url: "s3://my-bucket/events/"
    resources:
      - name: raw_events
        description: Events from the event export files
        reader: csv
        write_disposition: append
```

`filesystem` requires `bucket_url` in `config`.

### Resource options

| Key | Description |
|-----|-------------|
| `name` | Source name SQLBuild exposes (referenced via `__source("name")`). Required. |
| `description` | What the loaded rows are. Required ([Descriptions](../concepts/models/configuration.md#descriptions)). |
| `table` | Source table to replicate (`sql_database` only). Required for that type. |
| `endpoint` | Endpoint mapping (`rest_api` only). Required for that type. |
| `reader` | File reader: `csv`, `jsonl`, or `parquet` (`filesystem` only). Required for that type. |
| `write_disposition` | dlt write disposition: `replace`, `append`, or `merge`. `merge` requires `primary_key`. |
| `primary_key` / `merge_key` | dlt keys for `merge`. A `merge_key` replaces every row whose key value arrives, like a SQL model's `delete_insert` with [`replace_key`](../concepts/incremental.md#delete_insert). |
| `incremental` | dlt incremental config mapping (e.g. cursor column and start value). |
| `schema` | Per-resource schema override (falls back to the group `schema`). |

A `dlt_sources` entry accepts only `type`, `config`, `destination`, `schema`, and `resources`, and each resource accepts only the keys above for its source type. Any other key fails compilation with the nearest supported key as a hint.

Use SQLBuild's `write_disposition` (dlt's term), not `write_strategy`. `delete_insert` is not available declaratively; use a Python loader for that. Config values support SQLBuild's [interpolation](../concepts/interpolation.md): `${name}` for a project variable, `${ENV:NAME}` for an environment variable, and `${CTX:...}` for context, resolved at load time, so credentials stay out of the YAML.

### Destination

By default each resource loads into the database SQLBuild's adapter manages, with the dataset/schema derived from your target. On SQLite, dlt writes through its `sqlalchemy` destination into the project's database file (dataset `main`); the `sqlbuild[dlt]` extra installs the SQLAlchemy it needs. An optional `destination` mapping passes extra settings to the dlt destination, but it cannot set `credentials`, `dataset_name`, or `default_schema_name` (SQLBuild manages those):

```yaml
dlt_sources:
  - type: rest_api
    destination:
      loader_file_format: parquet
    config:
      client:
        base_url: https://api.example.com/
    resources:
      - name: raw_widgets
        endpoint:
          path: widgets
```

### Reference in models

Declared resources are managed sources, reference them like any other source:

```sql
SELECT id, title, state FROM __source("raw_github_issues")
```

## dlt inside a Python loader

When you need dlt capabilities beyond the declarative surface (custom transforms, `delete_insert`, sources not covered above), wrap a dlt pipeline in a [source loader](../concepts/python-nodes/loaders.md). The loader calls `dlt.pipeline(...).run(...)` and returns `None`; dlt handles the writes and SQLBuild treats the source as loaded.

```python
# python/loaders/github_sources.py
import dlt
from dlt.sources.rest_api import rest_api_source
from sqlbuild.loaders import loader
from sqlbuild.executor.load.models import LoaderContext

@loader
def raw_github_issues(ctx: LoaderContext):
    """Github issues."""
    source = rest_api_source({
        "client": {
            "base_url": "https://api.github.com/",
            "headers": {"Authorization": f"Bearer {ctx.vars['github_token']}"},
            "paginator": {"type": "header_link"},
        },
        "resources": [{
            "name": "issues",
            "endpoint": {
                "path": "repos/{owner}/{repo}/issues",
                "params": {
                    "owner": ctx.vars["github_owner"],
                    "repo": ctx.vars["github_repo"],
                    "state": "all",
                    "per_page": 100,
                },
            },
        }],
    })

    pipeline = dlt.pipeline(
        pipeline_name="github_issues",
        destination=dlt.destinations.duckdb(ctx.connection),
        dataset_name=ctx.destination_schema or "main",
    )
    pipeline.run(source)
```

Bind it to a source with `managed: true` in `sources/*.yml`:

```yaml
# sources/github.yml
sources:
  - name: raw_github_issues
    description: Github issues
    managed: true
    table: issues
    columns:
      - name: id
        type: INTEGER
      - name: title
        type: VARCHAR
```

### DuckDB connection sharing

With the DuckDB adapter, pass `ctx.connection` to dlt's DuckDB destination to reuse SQLBuild's open connection, so dlt writes into the same database without a separate connection string:

```python
pipeline = dlt.pipeline(
    pipeline_name="my_pipeline",
    destination=dlt.destinations.duckdb(ctx.connection),
    dataset_name=ctx.destination_schema or "main",
)
```

### Warehouse destinations

For Snowflake, BigQuery, or Databricks, configure dlt with its own credentials. dlt writes directly to the warehouse, and SQLBuild reads the resulting tables as sources:

```python
pipeline = dlt.pipeline(
    pipeline_name="api_data",
    destination="snowflake",
    dataset_name=ctx.destination_schema or "public",
)
```

Configure dlt credentials via its own `secrets.toml` or environment variables, as in the [dlt documentation](https://dlthub.com/docs/general-usage/credentials/setup).

## Build integration

Whether declarative or Python, loaders run automatically during `sqb build` (when `auto_load_sources` is enabled), so dlt pipelines execute as part of the normal build lifecycle:

```bash
sqb build            # loaders run, then models build
sqb build --no-load  # skip loading, use existing source data
sqb load             # run loaders standalone
```

See [Loaders](../concepts/python-nodes/loaders.md) for write strategies, the loader context API, auto-load behavior, and source deferral.
