<!-- generated-by: sqlbuild skills -->

# sqb clone

> Copy model relations between configured targets.

Online: https://sqlbuild.com/docs/cli/clone/

Copies selected relations from one target to another. It uses adapter-native cloning where supported and physical copies where required; `--hard-copy` forces a physical copy on adapters that support both.

No `manifest.json` generation or artifact management is required. Clone works directly against live targets.

When `--to` is omitted, the destination is the active target selected by `sqlbuild_local.toml`
or `default_target`, in that order.

`--from` selects only the origin namespace and its clone-origin policy. `--to`, or the active
target when it is omitted, selects the sole physical connection used by the operation. The
origin namespace must therefore be visible and readable through the destination connection.
SQLBuild never resolves or opens the origin target's connection, including for direct and
deferred clone. Clone cannot perform a cross-account, cross-server, or cross-file
transfer by opening a second connection.

Clone never runs SQL analysis: it reads relation names, dependencies, and rendered view and
function SQL from each target's project, so SQL analysis errors never stop it; structural errors,
such as an unknown reference, still do. The hidden `--no-sql-analysis` flag (alias
`--no-sql-validation`) is still accepted but has no effect.

With `--debug`, clone logs how long each phase took: compiling each target it reads, inspecting
sources, planning, and execution. The `--json-output` document reports the same durations in
seconds under `phase_timings`.

## Usage

```bash
sqb --project-dir <path> clone --from <target> [--to <target>] [flags]
```

## Flags

| Flag | Description |
|------|-------------|
| `--from` | Source target (required) |
| `--to` | Destination target; defaults to the active target |
| `--hard-copy` | Force physical table copies instead of zero-copy cloning |
| `--select`, `-s` | Select specific models to clone |
| `--exclude` | Exclude specific models from cloning |
| `--warehouse <name>` | Snowflake warehouse for this invocation; overrides the target's `build` [warehouse group](../concepts/adapters/snowflake.md#command-group-warehouses) and the connection warehouse |

## Examples

```bash
# Clone all models from prod to dev
sqb clone --from prod --to dev

# Equivalent when dev is the active target
sqb clone --from prod

# Clone only marts to dev
sqb clone --from prod --to dev --select path:models/marts

# Force physical copies
sqb clone --from prod --to dev --hard-copy
```

## Managed sources

Clone includes selected managed physical sources when the destination target reads its own
loader namespace. Sources are copied before seeds and models so views can be recreated in a
destination that has never been built.

Source locations respect target configuration:

- The origin relation comes from the target that the origin uses for managed source reads.
- The destination relation uses the destination target's `loader_schema`, falling back to
  its model `schema`.
- If the destination defers source reads to another target, clone reuses that target's source
  relation and does not copy or overwrite it.
- Expression sources and unmanaged external sources are not cloned.

`--hard-copy` controls how included relations are copied. It does not change which managed
sources are selected.

## Clone policies

Targets deny cloning by default. Enable the origin and destination explicitly in `sqlbuild_project.toml`:

```toml
[targets.prod.clone]
allow_as_clone_origin = true

[targets.dev.clone]
allow_as_clone_destination = true
```

The origin target must already contain the built relations being cloned. The destination
connection's credentials must be able to read the origin namespace and create relations in the
destination. Managed physical sources may bootstrap a destination that has not been built. See
[Configuration reference](../concepts/project-configuration.md) for details.
