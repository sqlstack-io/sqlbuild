<!-- generated-by: sqlbuild skills -->

# RuleContext and compiler facts

> Inspect stable compiler-owned facts and opt into lazy SQL AST access.

Online: https://sqlbuild.com/docs/concepts/rules/custom-rules/rule-context/

## Contents

- Authored and expanded SQL
- Common SQL structures
- Declared enum types
- Syntax tree
- Project structure
- Choose the subject deliberately

`RuleContext` exposes typed, read-only views over the compiled project:

- `ctx.sql`: authored and expanded SQL
- `ctx.graph`: compiler-resolved dependencies and dependents
- `ctx.columns`: declared and inferred output columns
- `ctx.contracts`: enforcement and grain facts
- `ctx.tests` and `ctx.audits`: checks associated with a model
- `ctx.declarations`: public and model-scoped enums and constants
- `ctx.project`: compiled resources and deterministic project-tree observations

## Authored and expanded SQL

Choose the SQL representation that matches the requirement:

```python
sql = ctx.sql.for_model(model)

authored_text = sql.authored.source
expanded_text = sql.expanded.source
```

An authoring convention may inspect what a developer wrote. A compiler-output convention may need
the expanded SQL after interpolation, declarations, and macros.

## Common SQL structures

Common structures are available on expanded SQL as `SqlNode` source locations, each with a `kind`,
a `line` and a `column`. The `kind` is the syntax tree's, such as `cte`, `join`, `subquery` or
`star`:

```python
for cte in sql.expanded.ctes():
    ...

for star in sql.expanded.star_projections():
    ...
```

| Method | Finds |
| --- | --- |
| `ctes()` | Each common table expression, at its name |
| `joins()` | Each explicit join clause, at its join keyword, not a comma-separated `FROM` item |
| `subqueries()` | Each parenthesized subquery, at its opening parenthesis |
| `star_projections()` | Each `*` and `table.*` that expands columns, including a function argument such as `OBJECT_CONSTRUCT(*)`, but not an aggregate's `*`, as in `COUNT(*)` |
| `nodes(kind=...)` | Each syntax tree node of exactly that kind, such as `column` |

`line` and `column` count within the document's `source`. For expanded SQL that is the query
text, not the model file. Authored SQL is the whole model file, including its `MODEL` header, so it
does not parse as SQL; read its text through `source`.

`ctx.finding(subject=model, node=node)` reports a node of that model's expanded SQL where it is in
the model file. A node inside a macro's output is reported at the macro call.

## Declared enum types

`ctx.columns.declared(model)` reports each declared column with its portable scalar type, so an
enum-typed column reads as `VARCHAR` or `INTEGER`. `ctx.columns.enum_types(model)` maps each
enum-typed column's name to the enum it declares:

```python
enum_types = ctx.columns.enum_types(model)
if enum_types.get("fulfillment_method") != "fulfillment_method":
    ...
```

## Syntax tree

Use the syntax tree only when the common structures are insufficient. Its nodes carry the names,
literal values and source spans a rule reads:

```python
sql = ctx.sql.for_model(model).expanded
for node in sql.ast().walk():
    if node.kind == "alias" and node.quoted and node.name != node.name.lower():
        start, end = node.span
        return [ctx.finding(subject=model, node=node, message=sql.source[start:end])]
```

`ast()` returns a `document` node whose children are the statements. Every `SqlTreeNode` has a
`kind`, a `line` and `column` where it starts, `children`, and `walk()`, which yields the node and
its descendants in pre-order. Kinds are SQLQuack's snake_case names, such as `select`, `subquery`,
`star`, `join`, `cte`, `alias`, `column`, `table`, `literal`, `function` and `window_function`.

A statement's clauses are nodes too, each owning the expressions written in it, so a rule can look
at one clause only:

```python
sql = ctx.sql.for_model(model).expanded
for clause in sql.ast().walk():
    if clause.kind == "where":
        columns = [node for node in clause.walk() if node.kind == "column"]
```

| Clause kind | Belongs to | Children |
| --- | --- | --- |
| `cte` | A statement with `WITH` | The CTE's query |
| `projection` | `select` | Each select-list item |
| `from` | `select` | Each `FROM` source, including a comma-separated one |
| `join` | `select`, `update`, `delete` | The joined source, then its `ON` condition |
| `where` | `select`, `update`, `delete` | The predicate |
| `group_by` | `select` | Each grouping key |
| `having` | `select` | The predicate |
| `qualify` | `select` | The predicate |
| `order_by` | `select`, `union`, `intersect`, `except`, `update`, `delete` | Each sort key, without its direction |
| `limit` | The same | The row count |
| `partition_by` | `window_function` | Each partition key |
| `window_order_by` | `window_function` | Each sort key, without its direction |

A clause appears only where the SQL writes it, except `projection`, which every `select` has. An
explicit join is a `join` node beside `from`. The `ORDER BY` and `LIMIT` after a `UNION`,
`INTERSECT` or `EXCEPT` belong to the set operation, not to its last `select`.

A statement's clause nodes come first among its children, in the order they are written: its
`cte` nodes, `projection`, `from`, each `join`, then `where`, `group_by`, `having`, `qualify`,
`order_by` and `limit`. Children outside any clause follow them, such as a set operation's queries,
which therefore come after its `order_by` and `limit`.

`cte` and `join` nodes have their own spans. The other clause nodes run from the start of their
first child to the end of their last, among children with spans, so a `where` node covers the
predicate. A clause none of whose children has a span has none either, and reports its
statement's start.

Nodes also have these attributes, which are `None` on nodes they don't apply to:

| Attribute | Nodes | Value |
| --- | --- | --- |
| `name` | `identifier`, `column`, `alias`, `table`, `cte` | The name without quotes, such as `order_id` |
| `quoted` | The same | Whether the source quotes the name |
| `qualifier` | The same | The names before it, outermost first: `("db", "sales")` for `db.sales.orders` |
| `value` | `literal` | The value as written, without quotes: `it's` for `'it''s'` |
| `literal_type` | `literal` | `string`, `number`, `national_string`, `date`, `timestamp` and the other literal types |
| `end_line`, `end_column` | Nodes with a source span | Where the node ends, just after its last character |
| `span` | Nodes with a source span | `(start, end)` character offsets into `source`, end exclusive |

A relation marker, such as `__ref("orders")`, reads as a `table` node whose `name` and `span` are
the marker call as written.

Parsing is lazy. A path or contract Rule does not pay for SQL parsing merely because syntax tree
access exists elsewhere.

The tree comes from the SQLQuack parser SQLBuild uses for the project's dialect:

| Dialect | Syntax tree |
| --- | --- |
| PostgreSQL | Native PostgreSQL parser |
| DuckDB, MotherDuck, Snowflake, SQLite | SQLQuack's shared parser |
| Other dialects | Unavailable |

A node without a source span has no end: `end_line`, `end_column` and `span` are `None`, and it
reports its parent's start. Literals have no span. Under the shared parser, neither does the query
inside a subquery.

Reading the tree, or any common structure, raises `RuleUsageError` when:

| Cause | Message starts with |
| --- | --- |
| The dialect has no syntax tree | `SQL syntax trees are unavailable for the <dialect> dialect` |
| The SQL does not parse | `SQL could not be parsed:` |
| A statement uses syntax the parser keeps as unparsed text, or nests too deeply | `SQL syntax tree is unavailable:` |

## Project structure

Use the compiler-owned tree rather than direct filesystem calls:

```python
parts = ctx.project.tree.relative_parts(path=model.path, under="models")
models = ctx.project.tree.resources_under("sql/models/orders")
config = ctx.project.tree.read_text("rules/requirements.yaml")
```

Paths remain project-relative, and observations participate in cache invalidation.

## Choose the subject deliberately

A model-subject Rule is evaluated and cached per model. A project-subject Rule runs once and can
iterate `ctx.project.models`. Use a project subject only when the invariant genuinely needs a
project-wide view; its cache invalidation is intentionally broader.
