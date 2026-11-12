<!-- generated-by: sqlbuild skills -->

# sqb contract

> Compare or generate repository contracts from physical warehouse schemas.

Online: https://sqlbuild.com/docs/cli/contract/

`sqb contract` compares a model's or source's declared columns with the table that already exists in
the warehouse, or writes declarations from it. It reads the warehouse and never changes it. The
target named by `--from` supplies the database and schema to read, using the project's connection.

## diff

```bash
sqb contract diff --from <target> [flags]
```

Reports declaration differences. Exit code `0` means agreement and `1` means differences exist.

## generate

```bash
sqb contract generate --from <target> [flags]
```

Prints proposed declaration updates without changing files.

```bash
# Fill in missing types and add warehouse columns missing from code
sqb contract generate --from prod --select tag:commerce --write

# Also replace conflicting types and names, and remove columns the warehouse doesn't have
sqb contract generate --from prod --select tag:commerce --write --overwrite
```

Without `--overwrite`, generation keeps everything you wrote: existing types where they conflict,
columns only in code, comments, descriptions, audits, nullability, enums, tags, freshness and loader
settings. It never adds `contract enforced`; that stays your decision.

A shared `SCHEMA()` isn't changed from one model's columns; the conflict is reported instead. Writes
are atomic, and SQLBuild recompiles the project afterwards and restores the old files if it fails.

## Flags

| Flag | Commands | Description |
|------|----------|-------------|
| `--from` | both | Target namespace to inspect (required) |
| `--json` | both | Print structured findings |
| `--select`, `-s` | both | Inspect selected models or sources |
| `--select-file` | both | Read selectors from a file |
| `--exclude` | both | Exclude resources from the selected scope |
| `--vars` | both | Override project variables |
| `--write` | generate | Apply generated repository changes |
| `--overwrite` | generate | Replace conflicting declarations; requires `--write` |
| `--warehouse <name>` | both | Snowflake warehouse for this invocation; overrides the target's `query` [warehouse group](../concepts/adapters/snowflake.md#command-group-warehouses) and the connection warehouse |

Sources can be selected like models, for example `--select source:raw_orders`. See
[Contracts](../concepts/models/contracts.md) for what a contract checks.
