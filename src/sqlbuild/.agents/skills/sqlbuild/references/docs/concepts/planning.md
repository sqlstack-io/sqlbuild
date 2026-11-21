<!-- generated-by: sqlbuild skills -->

# Planning and change detection

> How SQLBuild explains build work, detects changes, and chooses safe model actions.

Online: https://sqlbuild.com/docs/concepts/planning/

## Contents

- Reading a plan
- What SQLBuild compares
- Change reasons
- Replay and incremental actions
- Selection and coherence
- Recorded state
- Related topics

`sqb plan` previews the work for a build without executing it. `sqb build` uses the same planner
before running the selected resources.

Planning and selection are separate concerns:

- **Selection** determines which resources are in scope. By default, that is the whole project; use
  `--select` and `--exclude` to choose a smaller scope.
- **Change detection** compares compiled resources with recorded warehouse state. It explains why a
  resource needs work, chooses the correct action for incremental models, propagates upstream
  changes, and warns when a partial selection would be incoherent.

A build runs its selected scope. Change detection still matters: the plan can show query and
configuration differences, calculate replay or full-refresh actions, and identify stale upstreams.
It does not filter a build to stale models.

## Reading a plan

Run the plan command with the same target and selectors you intend to build:

```bash
sqb plan
sqb plan --target dev --select +fact_orders
```

A plan answers four questions:

1. Which resources are selected?
2. What does SQLBuild know about their current warehouse state?
3. Why does each selected model need work?
4. What materialization or incremental action will the build take?

Plans may include query diffs, schema changes, backfill ranges, source freshness signals, change
causes, and warnings about stale dependencies. Planning does not modify model relations.

## What SQLBuild compares

### Models and functions

Each model and function has a version identity derived from the inputs that affect its result:

- normalized SQL after macro expansion and reference resolution;
- materialization settings, contracts, rendered SQL hooks, Python hook versions, and other
  version-relevant configuration;
- referenced function identities; and
- upstream version identities where dependency changes must propagate.

A function change therefore appears on the function itself and on dependent models.

### Seeds

Seeds use a content hash plus load-affecting configuration. SQLBuild can distinguish an unchanged
seed from one whose file or loading behavior changed.

### Python nodes

Loaders, assets, checks, and Python hooks have identities derived from project-owned source,
transitive project dependencies, and decorator configuration. Python hook identities also
participate in the identity of models that invoke them.

For standalone Python nodes, this identity is primarily planning evidence. SQLBuild cannot observe
arbitrary external inputs such as APIs or files, so a node controls its own no-work decision with
`ctx.skip()` rather than relying on SQL identity alone.

### Sources

[Source freshness](planning/source-freshness.md) records an observable data version for a
source. A new observation can mark downstream models as affected even when their SQL is unchanged.

## Change reasons

The plan assigns reasons to explain detected work:

| Reason | Meaning |
|--------|---------|
| First run | No previous identity exists for the resource |
| Query changed / checksum changed | Compiled SQL or seed content differs from the recorded identity |
| Config changed | Version-relevant configuration differs |
| Schema changed | A relevant warehouse or upstream schema changed |
| Function changed | A function the model calls directly has a different definition |
| Renamed | The model's relation moves from an old name through a [model migration](models/migrations.md) |

These reasons explain the plan; they are not a promise that a build will skip entries labelled
current. Builds execute the selected scope.

## Replay and incremental actions

Each model's action comes from its own change. The resulting action depends on the
materialization:

- views are recreated when selected;
- tables rebuild when selected;
- incremental models use their cursor state and their own `replay_on_change` setting to determine
  the affected range; and
- a changed, new, renamed, or rebuilt upstream never replays a downstream incremental model, which
  continues forward from its cursor.

See [Replay decisions](planning/replay-decisions.md) for the detailed rules.

## Selection and coherence

SQLBuild reasons about dependencies outside a scoped selection without silently adding them to the
plan. If building the selected resources would use a stale or missing upstream, the planner warns or
blocks instead of presenting an incoherent partial build as current.

See [Selection and staleness](planning/selection-and-staleness.md) for closure selectors and
stale-upstream handling.

## Recorded state

SQLBuild records append-only planning evidence in the warehouse:

- `_sqlbuild_fingerprints` stores resource identities;
- `_sqlbuild_source_freshness` stores source observations; and
- `_sqlbuild_node_results` stores Python node outcomes.

The planner reads the latest applicable facts and appends new facts after successful work.

On Snowflake, SQLBuild creates its state tables as permanent tables with the maximum time travel:
90 days, or 1 day on editions that allow only 1. The retention is set when SQLBuild creates the
table. See [Snowflake state tables](adapters/snowflake.md#state-tables).

## Related topics

- [Replay decisions](planning/replay-decisions.md)
- [Source freshness](planning/source-freshness.md)
- [Selection and staleness](planning/selection-and-staleness.md)
