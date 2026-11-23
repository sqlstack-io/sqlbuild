<!-- generated-by: sqlbuild skills -->

# sqb init

> Scaffold a new SQLBuild project.

Online: https://sqlbuild.com/docs/cli/init/

Creates a new SQLBuild project with a minimal directory structure and configuration files.

## Usage

```bash
sqb init
```

No flags. Run in the directory where you want to create the project.

## Project layout

`sqb init` creates the configuration, empty resource directories needed for a standalone project,
and the SQLBuild agent skill (see [`sqb skills`](skills.md)):

```text
my-project/
  sqlbuild_project.toml
  .agents/skills/sqlbuild/
  .claude/skills/sqlbuild/
  models/
    staging/
    marts/
  schemas/
  sources/
  seeds/
  python/
  hooks/
    sql/
    python/
  tests/
    unit/
    scenarios/
  functions/
    sql/
    python/
  macros/
  audits/
    generic/
    singular/
```

Empty directories contain `.gitkeep` files so the scaffold can be committed. Reusable attached
audit definitions belong in `sql/audits/generic/`; cross-resource singular audits belong in
`sql/audits/singular/`. Add reusable SQL lifecycle hooks to `sql/hooks/sql/` and decorated Python lifecycle
hooks to `sql/hooks/python/`; see [Hooks](../concepts/models/hooks.md).

These top-level roles are project-wide. Once an audit, schema, hook, or macro is used only under
one folder, such as `sql/models/marts/`, SQLBuild requires it to move into that folder's `_sqlbuild/`
directory; see [Where to Put Declarations](../concepts/declaration-scopes/placement.md).

The generated project uses DuckDB, creates a named `developer` connection shared by `dev` and
`prod`, defaults models to table materialization, and makes audits warn by default. Its
configuration follows this shape:

```toml
name = "my_project"
adapter = "duckdb"
default_target = "dev"

[connections.developer]
database = "my_project.duckdb"

[settings]
default_audit_severity = "warn"

[defaults]
materialized = "table"

[targets.prod]
connection = "developer"
schema = "prod"

[targets.dev]
connection = "developer"
schema = "dev"
```

The project name is derived from the current directory name, with hyphens converted to
underscores.
