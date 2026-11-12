<!-- generated-by: sqlbuild skills -->

# sqb seed

> Load seed CSV files into the warehouse.

Online: https://sqlbuild.com/docs/cli/seed/

Loads seed CSV files into the warehouse as tables. Seeds are fully replaced on every run.

## Usage

```bash
sqb seed [SELECTOR ...] [flags]
```

Positional `SELECTOR` arguments work like `--select`, and both forms can be combined. `--project-dir <path>`, `--no-color`, and `--debug` can go before or after the command name.

## Flags

| Flag | Description |
|------|-------------|
| `SELECTOR ...` | Positional selectors, the same as `--select` |
| `--select`, `-s` | Select specific seeds by name |
| `--exclude` | Exclude specific seeds |
| `--warehouse <name>` | Snowflake warehouse for this invocation; overrides the target's `build` [warehouse group](../concepts/adapters/snowflake.md#command-group-warehouses) and the connection warehouse |

## Examples

```bash
# Load all seeds
sqb seed

# Load a specific seed
sqb seed seed:waffle_types
```
