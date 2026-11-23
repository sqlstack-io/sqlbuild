<!-- generated-by: sqlbuild skills -->

# Model-Private Values

> Keep enums and constants inside one model when no other resource should use them.

Online: https://sqlbuild.com/docs/concepts/model-private-values/

Put an enum or constant directly in `MODEL()` when it belongs to one model and should not enter the
project-wide namespace.

```sql
MODEL (
  description "One row per order",
  enums (
    _state [OPEN, CLOSED],
  ),
  constants (
    _min_items 2,
    _supported_countries ["GB", "FR", "DE"],
  ),
);

SELECT *
FROM order_batches
WHERE state = @enum("_state").OPEN
  AND item_count > @const("_min_items")
  AND country_code IN @const("_supported_countries")
```

## Where private values work

A model-private value is available in:

- The owning model's query
- Inline SQL hooks written in that model

It is not available in:

- Another model
- A child or sibling model directory
- A unit test or scenario
- A named SQL hook stored under `sql/hooks/sql/`

Private names begin with exactly one `_`. Names beginning with `__` are reserved for SQLBuild.
Because the model owns the name, different models may each define `_state` without creating a
collision.

## Explicit types and rendering

Use `constant(...)` when a private constant needs an exact type or rendering choice:

```sql
MODEL (
  description "Example model",
  constants (
    _usd_rate constant(
      type decimal,
      value "2.4700",
    ),
    _supported_countries constant(
      value ["GB", "FR", "HK"],
      render_as array,
    ),
  ),
);
```

## When a value becomes shared

Move the declaration out of `MODEL()` when another resource needs it. Remove the `_` prefix and put
it in the narrowest suitable enum or constant directory. See
[Declarations and Scopes](declaration-scopes.md) for those advanced placement options.
