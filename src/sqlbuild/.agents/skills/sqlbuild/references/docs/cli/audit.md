<!-- generated-by: sqlbuild skills -->

# sqb audit

> Run data quality audits in isolation.

Online: https://sqlbuild.com/docs/cli/audit/

Runs all attached audits without rebuilding models. Useful for verifying data quality on existing warehouse state.

## Usage

```bash
sqb audit [SELECTOR ...] [flags]
```

Positional `SELECTOR` arguments work like `--select`, and both forms can be combined. `--project-dir <path>`, `--no-color`, and `--debug` can go before or after the command name.

## Flags

| Flag | Description |
|------|-------------|
| `--no-sql-analysis` | Disable compile-time SQL analysis (`--no-sql-validation` is an alias) |
| `--defer-to` | Resolve model references against another target |
| `SELECTOR ...` | Positional selectors, the same as `--select` |
| `--select`, `-s` | Select audits attached to specific models |
| `--select-file` | Read selectors from a file |
| `--exclude` | Exclude audits attached to specific models |
| `--concurrency` | Maximum number of audits to run concurrently (default: `1`) |
| `--json` | Print structured audit results |
| `--json-output` | Write structured audit results to a file |
| `--warehouse <name>` | Snowflake warehouse for this invocation; overrides the target's `build` [warehouse group](../concepts/adapters/snowflake.md#command-group-warehouses) and the connection warehouse |

## Concurrency

Standalone audits run serially by default. Use `--concurrency` to opt into parallel warehouse
queries:

```bash
sqb audit --concurrency 8
```

SQLBuild resolves the limit in this order: an explicit `--concurrency` value, the
`SQLBUILD_CONCURRENCY` environment variable, the effective project `settings.concurrency`, then
the product default of `1`. Values must be at least `1`.

The physical worker count is bounded by the number of selected audits. Each active worker opens
and exclusively uses one warehouse connection, so higher concurrency can increase warehouse load
and cost. Results and final JSON remain in plan order even when queries finish in a different
order.

If an audit query raises a runtime, adapter, or framework error, SQLBuild records that failure and
continues running the remaining independent audits. After all selected audits have been attempted,
it reports every query failure in plan order and exits nonzero. Failed queries do not produce
fabricated audit results.

Interruption and cancellation are different: SQLBuild stops scheduling new audits, drains queries
that have already started, and then closes every worker connection. Generic adapters cannot
guarantee immediate cancellation of a query already running in the warehouse.

## Examples

```bash
# Run all audits
sqb audit

# Run audits for marts only
sqb audit path:models/marts

# Run up to 8 audit queries concurrently
sqb audit --concurrency 8
```
