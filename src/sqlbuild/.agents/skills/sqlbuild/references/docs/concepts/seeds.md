<!-- generated-by: sqlbuild skills -->

# Seeds

> Load static CSV data into your pipeline as tables.

Online: https://sqlbuild.com/docs/concepts/seeds/

## Contents

- Defining a seed
- Referencing seeds
- Loading seeds
- Seed audits
- Target overrides
- CSV settings

Seeds are CSV files under `sql/seeds/` that SQLBuild loads as tables in the warehouse. They're useful for small, static reference data like lookup tables, category mappings, or configuration values.

## Defining a seed

Place a CSV file in your `sql/seeds/` directory:

```csv
waffle_type_id,waffle_name,category,price_cents
1,Classic Belgian,sweet,850
2,Liege,sweet,950
3,Brussels,sweet,750
4,Cheddar Herb,savory,1050
```

Declare the seed's column types in any `.yml` file under `sql/seeds/`. You can use any filename and organize declarations however you like - one file per seed, or group related seeds together:

```yaml
# sql/seeds/lookups.yml
seeds:
  - name: waffle_types
    description: Waffle menu items with pricing.
    columns:
      - name: waffle_type_id
        type: INTEGER
      - name: waffle_name
        type: VARCHAR
      - name: category
        type: VARCHAR
      - name: price_cents
        type: INTEGER
```

Seed YAML accepts only these keys; any other key is a compile error:

| Level | Keys |
|-------|------|
| File | `seeds` |
| Seed | `name`, `description`, `database`, `schema`, `meta`, `tags`, `csv_settings`, `columns`, `audits` |
| Column | `name`, `type`, `nullable`, `description`, `meta`, `audits` |

Every seed must have a YAML declaration with a non-empty `description` ([Descriptions](models/configuration.md#descriptions)) and at least one typed column. CSV filenames must be unique across the entire `sql/seeds/` directory (including subdirectories). The seed name in the YAML must match the CSV filename (without the `.csv` extension).

## Referencing seeds

Seeds are referenced in models with `__seed()`:

```sql
SELECT
  o.order_id,
  w.waffle_name,
  w.price_cents * o.quantity AS line_total_cents
FROM __ref("stg_orders") o
LEFT JOIN __seed("waffle_types") w ON o.waffle_type_id = w.waffle_type_id
```

`__seed()` is distinct from `__ref()`. Using `__ref()` with a seed name raises a compile error with a message pointing you to `__seed()`.

## Loading seeds

Seeds are loaded automatically during `sqb build`. You can also load them standalone:

```bash
sqb seed
```

Seeds are fully replaced on every run. If the CSV changes, the table is recreated with the new data.

## Seed audits

Attach audits to a seed or its columns with the same syntax as source audits:

```yaml
seeds:
  - name: waffle_types
    description: Waffle types
    columns:
      - name: waffle_type_id
        type: INTEGER
        audits:
          - not_null
          - unique
    audits:
      - expression_is_true:
          expression: "price_cents >= 0"
          severity: error
```

`not_null` and `unique` are built in. `expression_is_true` is a project generic audit; one that
works on seeds reads the attached relation through `@relation`:

```sql
-- sql/audits/generic/expression_is_true.sql
AUDIT ();

SELECT *
FROM @relation
WHERE NOT (@expression)
```

A seed audit receives `@seed` (the seed name) and `@relation`, but not `@model`, so a generic
audit written as `FROM __ref("@model")` works only on models.

During `sqb build`, seed audits run right after the seed loads. An `error` failure blocks every
model that depends on the seed. See [Audits](audits.md#seed-audits).

## Target overrides

Seeds inherit the project's default database and schema. You can override these per seed:

```yaml
seeds:
  - name: waffle_types
    description: Waffle types
    database: analytics
    schema: lookups
    columns:
      - name: waffle_type_id
        type: INTEGER
      - name: waffle_name
        type: VARCHAR
```

## CSV settings

For non-standard CSV formats, configure parsing behavior with `csv_settings`:

```yaml
seeds:
  - name: european_prices
    description: European prices
    csv_settings:
      delimiter: ";"
      encoding: utf-8
      quotechar: '"'
    columns:
      - name: product_id
        type: INTEGER
      - name: price
        type: VARCHAR
```

### Available CSV settings

| Setting | Type | Description |
|---------|------|-------------|
| `delimiter` | string | Field delimiter (default: `,`) |
| `quotechar` | string | Character used to quote fields (default: `"`) |
| `doublequote` | boolean | Whether the quotechar is doubled to escape itself |
| `escapechar` | string | Character used to escape the delimiter or quotechar |
| `skipinitialspace` | boolean | Skip whitespace after the delimiter |
| `lineterminator` | string | Line terminator character |
| `encoding` | string | File encoding (default: `utf-8`) |
| `na_values` | list or mapping | Values to treat as null - a global list or per-column mapping |
| `keep_default_na` | boolean | Whether to use the default set of NA values in addition to `na_values` |

### Per-column NA values

`na_values` can be a flat list (applied to all columns) or a mapping from column names to their NA values:

```yaml
csv_settings:
  na_values:
    price: ["N/A", ""]
    category: ["unknown"]
```
