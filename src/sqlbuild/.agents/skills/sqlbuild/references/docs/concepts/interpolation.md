<!-- generated-by: sqlbuild skills -->

# Variables and placeholders

> How SQLBuild processes variables, context, and dynamic content in SQL and config.

Online: https://sqlbuild.com/docs/concepts/interpolation/

## Contents

- Syntax reference
- Project variables
- Environment variables
- Context variables
- Deferred placeholders
- Audit parameters
- Named SQL hook parameters
- Compilation order

SQLBuild uses two syntax layers for dynamic content:

- **`@` syntax** is for any executable SQL - model queries, SQL hooks, tests, audits, and inline source expressions
- **`${...}` syntax** is for config values - project TOML config, MODEL() header fields (excluding SQL hooks), and source/seed YAML declarations

The rule is simple: if it's any SQL that will be executed, it uses `@`. If it's a config value, it uses `${...}`. These layers never mix.

## Syntax reference

| Syntax | Where | Resolved |
|--------|-------|----------|
| `@enum("name").MEMBER` | Executable SQL | Compile time - expands to the validated enum member value |
| `@const("name")` | Executable SQL | Compile time - expands to the named typed scalar, value list, native array, or object expression |
| `@sql.name(args)`, `@py.name(args)` | Model SQL, SQL hooks, tests, audits, inline source expressions | Compile time - expands to the macro's SQL |
| `@@name` | Model SQL, SQL hooks, tests, audits, inline source expressions | Compile time - project variable substitution |
| `@@ENV:NAME` | Model SQL, SQL hooks, tests, audits, inline source expressions | Compile time - environment variable |
| `@@CTX:name` | SQL hooks only | Compile time - destination relation, target, run ID |
| `@@@name` | Model SQL | Preserved for runtime (custom materializations) |
| `@name` / `@'name'` | Named SQL hook bodies | Raw / SQL-literal invocation argument, resolved at compile time |
| `@name` / `@'name'` | Generic audit SQL | Raw / SQL-literal audit argument |
| `${CTX:...}` | TOML/YAML config values | Config compilation |
| `${ENV:...}` | TOML/YAML config values | Config compilation |

`@@CTX:` is intentionally SQL-hook-only. Model SQL describes a relation's data and should not reference its own destination identity. SQL hooks are the operational SQL layer where destination context is useful - grants, logging, post-materialization DDL. Python hooks access the same information through `ctx.destination` on the [`HookContext`](models/hooks/python.md#hook-context) object.

See [Enums](enums.md) and [Constants](constants.md) for reusable validated values. See
[Collections and Rendering](constants/collections-and-rendering.md) for lists, sets, objects,
and adapter-specific rendering.

SQLBuild uses the location of a SQL file to determine which macros, enums, and constants it can
use. Named SQL hooks use their own file location rather than the location of the calling model. See
[How Visibility Works](declaration-scopes/visibility.md) for the complete directory rules.

## Project variables

Project variables use `@@name` syntax in SQL and are defined in `sqlbuild_project.toml` or per-target:

```toml
# sqlbuild_project.toml
[vars]
schema_prefix = "analytics"

[targets.prod.vars]
schema_prefix = "prod_analytics"
```

```sql
SELECT * FROM @@schema_prefix.customers
```

Variables can also be set in `sqlbuild_local.toml` for developer-specific overrides, or passed via the CLI using `--vars` with a JSON object:

```bash
sqb build --vars '{"schema_prefix": "staging_analytics"}'
```

CLI vars take precedence over local and project config vars.

### JSON vars and nested values

`--vars` accepts full JSON objects. Values can be strings, numbers, booleans, null, arrays, or nested objects:

```bash
sqb build --vars '{"schema_prefix": "staging", "grants": {"primary_role": "analyst"}, "enabled": true}'
```

In SQL interpolation (`@@name`), only top-level scalar values can be used directly. `null` renders as an empty string. If a variable resolves to an array or object, SQLBuild raises a clear error suggesting you use a macro instead.

For nested or complex values, use `ctx.vars` in a macro:

```python
def grant_role(ctx):
    return ctx.vars["grants"]["primary_role"]
```

The macro context receives the full native JSON structure including nested dicts, lists, booleans, numbers, and `None`.

## Environment variables

Environment variables use `@@ENV:NAME` syntax to inject values from the shell environment:

```sql
SELECT *
FROM @@schema_prefix.customers
WHERE source_system = '@@ENV:SOURCE_SYSTEM'
```

If the environment variable is not set, SQLBuild raises a compile error.

## Context variables

Context variables provide access to the current model's destination relation, active target, and run metadata.

**In SQL hooks** (`@@CTX:` syntax):

```sql
post_hooks [inline_sql('GRANT SELECT ON @@CTX:destination.qualified TO analyst_role')],
```

Context components can appear directly beside qualification dots. SQLBuild resolves the longest
available context key before treating the dot as SQL punctuation:

```sql
CREATE FUNCTION @@CTX:destination.database.@@CTX:destination.schema.reconstruct_book()
RETURNS INTEGER
LANGUAGE SQL
AS 'SELECT 1'
```

**In TOML/YAML config values** (`${CTX:...}` syntax):

```toml
[targets.prod]
schema = "${CTX:destination.schema}"
```

Available context variables:

| Variable | Value |
|----------|-------|
| `destination.qualified` | Fully qualified destination relation name |
| `destination.schema` | Destination schema |
| `destination.database` | Destination database |
| `destination.table` | Destination relation name |
| `model.name` | Model name |
| `model.database` | Model database |
| `model.schema` | Model schema |
| `model.alias` | Model alias |
| `run.target` | Active target name |
| `run.id` | Current run ID |

**In macros**, the `MacroContext` object is passed as the first argument when a macro function accepts a `ctx` parameter:

```python
def timestamp_trunc(ctx, grain: str, expr: str) -> str:
    if ctx.adapter_name == "bigquery":
        return f"TIMESTAMP_TRUNC({expr}, {grain.upper()})"
    return f"DATE_TRUNC('{grain}', {expr})"
```

The macro context provides `adapter_name`, `sql_analysis_enabled`, `target_name`, and `vars`.

## Deferred placeholders

Custom materializations can define runtime placeholders using `@@@name` syntax. These are preserved through compilation and resolved by the materialization at execution time:

```sql
WHERE CAST(ordered_at AS DATE) >= CAST(@@@partition_start AS DATE)
  AND CAST(ordered_at AS DATE) < CAST(@@@partition_end AS DATE)
```

## Audit parameters

Generic audit SQL uses `@name` (single `@`, no parentheses) for raw SQL placeholders and `@'name'` for escaped SQL-literal placeholders. These are resolved when the audit is attached:

```sql
SELECT @column
FROM @relation
WHERE @column IS NULL
```

Use the quoted form for values rather than SQL identifiers or expressions:

```sql
WHERE status = @'expected_status'
```

This is distinct from `@@name` (project variables) and `@sql.name()` or `@py.name()` (macro calls), so there is no ambiguity. See [Audits](audits.md) for details on generic audit parameters.

## Named SQL hook parameters

Reusable hooks under `hooks/sql/` use `@name` for raw SQL substitution and `@'name'` for escaped SQL-literal substitution. These parameters are supplied by `sql("name", args...)` and resolved before the rest of the hook SQL is compiled. See [SQL hooks](models/hooks/sql.md#sql-hook-arguments) for value rendering and validation rules.

## Compilation order

SQLBuild processes authored SQL in this order:

1. **Config templates** (`${CTX:...}`, `${ENV:...}`) in TOML/YAML config values are resolved during config compilation
2. **Named SQL hook arguments** (`@name` and `@'name'`) are substituted into reusable hook bodies
3. **Project variables** (`@@name`), **environment variables** (`@@ENV:NAME`), and **context variables** (`@@CTX:name` in SQL hooks) are substituted
4. **Enum and constant references** are validated, normalized, and expanded through the active adapter, and **SQL macro calls** (`@sql.name(args)`) are expanded
5. **Python macro calls** (`@py.name(args)`) are expanded
6. **SQL analysis validation** runs against the fully expanded SQL

This means:
- Config templates resolve first, before any SQL processing
- Named SQL hook arguments can contain `@@CTX:...`, `@@ENV:...`, project-variable, declaration, or macro text; model context values describe the invoking model, while declarations and macros in the resulting named hook body resolve from the hook definition path
- Macros see already-substituted variable values in the SQL
- `@@CTX:destination.qualified` in SQL hooks sees the final target-overridden destination name because hooks are expanded after destination naming is fully resolved
- SQL analysis validates the final expanded SQL, catching syntax errors from both vars and macros
- Python hook arguments are not SQL and remain unexpanded
