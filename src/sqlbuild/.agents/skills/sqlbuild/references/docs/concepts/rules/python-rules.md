<!-- generated-by: sqlbuild skills -->

# Python Rules

> Check the project's Python for annotations, imports, function shape and the layout of python/.

Online: https://sqlbuild.com/docs/concepts/rules/python-rules/

## Contents

- Selecting the Python Rules
- Which files are checked
- Laying out `python/`
- Writing nodes as units
- Main modules and entry modules
- Unreachable definitions
- Rule options
- Rule reference
- Details

The `SQBRPYTHON` Rules check the Python in your project the way `SQLCLEAN` checks its SQL. Select
the family and `sqb compile` reports each finding with its file, line and column:

```toml
[rules]
select = ["SQBR", "SQBRPYTHON"]
```

```text
error[SQBRPYTHON101]: function parameter 'ctx' must define a type annotation
  --> hooks/python/alert.py:4:11
  = help: Annotate every parameter with the value type accepted by the function.
```

## Selecting the Python Rules

The Python Rules run only when a selector names `SQBRPYTHON` or one of its codes. `SQBR` on its own
leaves the whole family out, so a project that selects `SQBR` keeps its current findings until it
adds `SQBRPYTHON`.

| Selection | Python Rules that run |
|---|---|
| no `select`, or `select = ["SQBR"]` | none |
| `select = ["SQBRPYTHON"]` | every Rule enabled by default |
| `select = ["SQBRPYTHON", "SQBRPYTHON306"]` | the defaults and `SQBRPYTHON306` |
| `select = ["SQBRPYTHON101"]` | `SQBRPYTHON101` only |

`SQBRPYTHON306` (unreachable definitions) and `SQBRPYTHON507` (parameter mutation) are off by
default. Select them by their exact code; the family selector does not include them.

`ignore` matches by prefix as usual, so `ignore = ["SQBR"]` also turns the Python Rules off.

Try the family before adding it to `select`:

```bash
sqb rules run SQBRPYTHON
```

## Which files are checked

The Python Rules read every supported project Python file: the runtime package under `python/` and
the folders that extend SQLBuild, such as `hooks/python/`, `functions/python/`, `providers/`,
`rules/` and `macros/`. See [Project Python](../project-configuration.md#project-python)
for the full list. Files under `tests/` are not checked.

Rules marked `python/` in the [reference](#rule-reference) check only the runtime package, because
they describe how its packages are organised. The others check every file.

## Laying out `python/`

`SQBRPYTHON001` and `SQBRPYTHON002` keep `python/` organised by what the code is for, not by what
kind of node it holds. Directly under `python/`, keep only domain packages, `utils/` for code no
domain owns, and `__init__.py`:

```text
python/
  __init__.py
  fulfillment/
    shipment_export/
      node.py          # @task shipment_export
      main.py          # main(), the unit's implementation
  utils/
    currency/
      format.py
```

These layouts are reported:

| Finding | Example | Fix |
|---|---|---|
| A module directly under `python/` | `python/report.py` | Move it into the domain that owns it |
| A folder named for a node kind or a role | `python/loaders/`, `python/_helpers/` | Group the nodes by domain instead |
| A package with a generic name | `python/inventory/common/` | Name it after what it owns |

The generic names are `base`, `common`, `general`, `helpers`, `lib`, `misc`, `shared`, `util` and
`utils`. `utils/` is allowed directly under `python/`. A finding about a folder points at its
`__init__.py`, or at its first Python file when it has none.

## Writing nodes as units

A unit is one folder that declares one node, or one factory, and holds its implementation.
`SQBRPYTHON003` to `SQBRPYTHON012` keep each node in its own unit, grouped by domain:

```text
python/orders/
  constants.py            # shared by the orders units
  partner_export/
    node.py               # the @asset declaration
    main.py               # main(), the implementation
    checks.py             # @check declarations for this unit
    _helpers/             # private functions for this unit
  refund_review/
    node.py               # @task
    main.py
```

A unit folder may also hold `models.py`, `types.py`, `constants.py`, `exceptions.py` and
`__init__.py`. The declaration only passes its arguments to `main()`:

```python
# python/orders/partner_export/node.py
from sqlbuild.assets import AssetContext, asset
from sqlbuild.executor.python_nodes.models import PythonNodeResult

from providers.partner_files.provider import PartnerFiles
from python.orders.partner_export.main import main

@asset(description="Write the daily partner orders file.")
def partner_export(ctx: AssetContext, *, partner_files: PartnerFiles) -> PythonNodeResult:
    return main(ctx=ctx, partner_files=partner_files)
```

These are reported:

| Code | Finding | Fix |
|---|---|---|
| `SQBRPYTHON003` | A unit without `main.py`, or another module or folder in it | Add `main.py`; move other code into `_helpers/` or a role file |
| `SQBRPYTHON004` | `node.py` holding a helper, a constant, a class or a second declaration | Keep one `@loader`, `@task`, `@asset` or `@factory` function and its imports |
| `SQBRPYTHON005` | A declaration that does more than `return main(...)`, or passes a positional argument | Move the logic into `main()` and pass every argument as `name=value` |
| `SQBRPYTHON006` | `main.py` holding more than one function, or a `main()` whose return annotation does not fit the node | Move other code into `_helpers/` and annotate `main()` with its result |
| `SQBRPYTHON007` | A signature other than `(ctx: <Kind>Context, *, provider: ProviderClass)` | Take `ctx` positionally and providers after `*`, each annotated |
| `SQBRPYTHON008` | An import of another unit's `main.py`, role files or `_helpers/`, or another domain's code | Import shared code from `python/utils/` or the domain's role files |
| `SQBRPYTHON009` | `checks.py` holding other code, or a check that does not depend on the unit's node | Name the unit's node in `depends_on` and return directly |
| `SQBRPYTHON010` | A factory that does not loop over the unit's specs, or a generated node without `name=` | Loop over specs from the unit's `constants.py` or `models.py` and name each node |
| `SQBRPYTHON011` | A docstring on a node or check, a task, asset or check without `description=`, or a described loader | Use `description=`; a loader takes its description from its managed source |
| `SQBRPYTHON012` | A node declared outside a unit's `node.py`, or a check outside its `checks.py` | Move it into its own unit folder |

`main()` returns:

| Node | Return annotation |
|---|---|
| `@task`, `@asset` | `PythonNodeResult` or `PythonNodeSkipResult`, from `ctx.result(...)` and `ctx.skip(...)` |
| `@loader` | rows such as `list[...]` or `Iterator[...]`, `LoaderResult`, `LoaderSkipResult`, or `None` |

A unit may import another unit's `node.py` only to name that node in `depends_on`. It may import
`provider.py`, `models.py`, `types.py`, `constants.py` and `exceptions.py` from
`providers/<capability>/`.

A factory's `node.py` holds one `@factory` function, and a check generated by a factory may stay in
`node.py`. `@audit_factory` functions are not units and may sit anywhere under `python/`.

## Main modules and entry modules

Some Rules apply only to a domain's main modules: files under a `main/` folder, and a unit's
`main.py`, such as `python/fulfillment/shipment_export/main.py`. A main module that is not
`__init__.py` is an entry module. `SQBRPYTHON617` requires it to expose one public function, and
`SQBRPYTHON305` requires something outside its domain to import it.

`SQBRPYTHON501` to `SQBRPYTHON503` (statements, distinct calls and locals) check main modules only.
The other size limits check every function.

## Unreachable definitions

`SQBRPYTHON306` reports functions, classes and module values that nothing SQLBuild loads can reach.
SQLBuild loads every decorated function under `python/` (`@loader`, `@task`, `@asset`, `@check`,
`@factory` and `@audit_factory`) and everything defined outside `python/`, such as hooks and
providers. A definition reached only through them is live. Imports through folders without an
`__init__.py` count too.

Code that SQLBuild loads in a way the Rule cannot follow, such as by name at run time, needs a rule
exception with a reason:

```toml
[[rules.rule_exceptions]]
rule = "SQBRPYTHON306"
path = "python/fulfillment/carriers/registry.py"
reason = "Carriers are looked up by name from the shipment's carrier code."
```

## Rule options

Configure the size limits by exact code:

```toml
[rules.rule_options.SQBRPYTHON504]
max_arguments = 6
```

| Rule | Option | Default | Meaning |
|---|---|---|---|
| `SQBRPYTHON501` | `max_statements` | 40 | Most statements one main-module function may hold. |
| `SQBRPYTHON502` | `max_distinct_calls` | 20 | Most distinct callees one main-module function may call. |
| `SQBRPYTHON503` | `max_locals` | 20 | Most local names one main-module function may bind. |
| `SQBRPYTHON504` | `max_arguments` | 10 | Most parameters one function may declare. |
| `SQBRPYTHON505` | `max_statements_global` | 70 | Most statements any function may hold. |
| `SQBRPYTHON610` | `max_helpers_container_modules` | 10 | Most modules one `_helpers/` package may hold directly. |
| `SQBRPYTHON610` | `max_role_depth` | 1 | How many package levels may sit below a role package. |
| `SQBRPYTHON611` | `max_main_container_modules` | 20 | Most modules one `main/` package may hold directly. |
| `SQBRPYTHON611` | `max_role_depth` | 1 | How many package levels may sit below a role package. |
| `SQBRPYTHON615` | `min_shared_domain_prefix_packages` | 2 | Fewest sibling domains sharing a name prefix that are reported; 0 turns the rule off. |
| `SQBRPYTHON627` | `max_file_lines` | 2000 | Most lines one Python file may hold. |

Each option takes a whole number of 0 or more. Unknown option names, text values and negative
numbers fail configuration with `C000`.

## Rule reference

`Default` says whether `SQBRPYTHON` selects the Rule. `sqb rules show <code>` prints the full
remediation.

### Layout of `python/`

| Code | Name | Files | Default | Checks |
|---|---|---|---|---|
| `SQBRPYTHON001` | project-root-allowlist | `python/` | yes | `python/` may hold only domain packages, `utils/` and `__init__.py` |
| `SQBRPYTHON002` | runtime-package-owner-names | `python/` | yes | Packages under `python/` must be named for what they own |

### Node units

| Code | Name | Files | Default | Checks |
|---|---|---|---|---|
| `SQBRPYTHON003` | node-unit-layout | `python/` | yes | A unit folder holds `node.py`, `main.py`, optional role files and `_helpers/` only |
| `SQBRPYTHON004` | node-module-shape | `python/` | yes | `node.py` holds imports and exactly one node or factory declaration |
| `SQBRPYTHON005` | node-delegates-to-main | `python/` | yes | A declaration's body is `return main(...)` from its unit, with keyword arguments |
| `SQBRPYTHON006` | main-module-shape | `python/` | yes | `main.py` holds imports and one `main()` whose return annotation fits the node |
| `SQBRPYTHON007` | node-signature | `python/` | yes | `ctx` is the only positional parameter; providers are annotated and keyword-only |
| `SQBRPYTHON008` | node-import-boundaries | `python/` | yes | A unit imports only what it owns, its domain's role files, `python/utils/`, provider role files and other nodes for `depends_on` |
| `SQBRPYTHON009` | checks-module-shape | `python/` | yes | `checks.py` holds checks of the unit's node that return directly, or one factory |
| `SQBRPYTHON010` | factory-shape | `python/` | yes | A factory loops over its unit's specs and names every node it generates |
| `SQBRPYTHON011` | node-description-not-docstring | `python/` | yes | Tasks, assets and checks set `description=` without a docstring; loaders set neither |
| `SQBRPYTHON012` | node-declared-in-unit | `python/` | yes | Nodes are declared in a unit's `node.py`, and checks in its `checks.py` |

### Type annotations

| Code | Name | Files | Default | Checks |
|---|---|---|---|---|
| `SQBRPYTHON101` | parameter-annotation | All | yes | Function parameters must define type annotations |
| `SQBRPYTHON102` | return-annotation | All | yes | Functions must define return type annotations |
| `SQBRPYTHON103` | module-variable-annotation | All | yes | Module-level variables must define type annotations |
| `SQBRPYTHON104` | class-attribute-annotation | All | yes | Class attributes must define type annotations |
| `SQBRPYTHON105` | local-variable-annotation | All | yes | Local variables must define type annotations on first binding unless assigned a scalar literal |

### Hygiene

| Code | Name | Files | Default | Checks |
|---|---|---|---|---|
| `SQBRPYTHON201` | single-line-docstrings | All | yes | Docstrings must be a single line; move extended explanation into docs or tests |
| `SQBRPYTHON202` | no-standalone-comments | All | yes | Standalone comments are not allowed; prefer clear names or docs/tests |
| `SQBRPYTHON203` | no-raw-builtin-raise | All | yes | Runtime code must raise structured errors instead of raw built-in exceptions |
| `SQBRPYTHON204` | no-assert-in-runtime | All | yes | Runtime code must not use assert for invariants; raise a structured error |
| `SQBRPYTHON205` | no-swallowed-exception-probe | All | yes | Runtime code must not swallow broad exceptions as existence probe answers |
| `SQBRPYTHON206` | no-unnamed-string-decisions | All | yes | String literals must not directly control comparison behavior |
| `SQBRPYTHON207` | no-magic-numeric-comparisons | All | yes | Non-canonical numeric literals must not directly control comparisons |
| `SQBRPYTHON208` | no-import-time-side-effects | All | yes | Runtime and tooling modules must not execute standalone calls during import |

### Imports and reachability

| Code | Name | Files | Default | Checks |
|---|---|---|---|---|
| `SQBRPYTHON301` | absolute-imports-only | All | yes | Use absolute imports; relative imports hide package boundaries |
| `SQBRPYTHON302` | no-star-imports | All | yes | Star imports hide names from dependency-boundary analysis |
| `SQBRPYTHON303` | no-internal-public-surface-imports | `python/` | yes | Internal code must import from the owning module, not the bare package |
| `SQBRPYTHON304` | no-cross-domain-private-main-imports | `python/` | yes | Domain-private main entries may only be imported within their owning domain |
| `SQBRPYTHON305` | public-main-entry-external-use | `python/` | yes | Public main entries must have an importer outside their owning domain |
| `SQBRPYTHON306` | unreachable-definition | `python/` | no | Definition is unreachable from production roots |
| `SQBRPYTHON307` | no-cross-file-use-of-helper-private-class | `python/` | yes | Helper-private classes are file-local details; move shared classes to `classes/` |

### Naming contracts

| Code | Name | Files | Default | Checks |
|---|---|---|---|---|
| `SQBRPYTHON401` | validator-must-not-return | All | yes | Functions under no-return naming contracts must not return values |
| `SQBRPYTHON402` | predicate-must-return-bool | All | yes | Predicate names must declare an ordinary boolean result |
| `SQBRPYTHON403` | value-name-must-return-value | All | yes | Value-producing names must not declare a no-value result |
| `SQBRPYTHON404` | iterator-name-must-produce-iterator | All | yes | Iterator names must produce an iterator or generator |

### Function shape

| Code | Name | Files | Default | Checks |
|---|---|---|---|---|
| `SQBRPYTHON501` | too-many-statements | All | yes | Main functions must stay phase-shaped and below the statement limit |
| `SQBRPYTHON502` | too-many-distinct-calls | All | yes | Main functions must not coordinate too many distinct callees |
| `SQBRPYTHON503` | too-many-locals | All | yes | Main functions must not juggle too many local variables |
| `SQBRPYTHON504` | max-arguments | All | yes | Functions must stay below the configured argument limit |
| `SQBRPYTHON505` | max-statements-global | All | yes | Functions must stay below the global statement limit |
| `SQBRPYTHON506` | meaningful-project-result-discarded | All | yes | Main orchestrators must consume meaningful project-local call results |
| `SQBRPYTHON507` | parameter-mutation-in-phase-helpers | All | no | Helpers must return values instead of mutating parameters |
| `SQBRPYTHON508` | default-mutation-return | All | yes | Functions that mutate parameters must return every mutated parameter |
| `SQBRPYTHON509` | no-outer-state-mutation | All | yes | Functions must not mutate module-global or closure-captured state |
| `SQBRPYTHON510` | no-complex-comprehensions | All | yes | Nested or multi-generator comprehensions hide control flow and data shapes |
| `SQBRPYTHON511` | mutable-result-model | All | yes | Dataclass result models must be frozen |

### Module roles and package layout

| Code | Name | Files | Default | Checks |
|---|---|---|---|---|
| `SQBRPYTHON601` | models-only-models | `python/` | yes | Models role files may contain only structured runtime models |
| `SQBRPYTHON602` | types-only-types | `python/` | yes | Types role files may contain only type-layer declarations |
| `SQBRPYTHON603` | constants-only-constants | `python/` | yes | Constants role files may contain only assignments and imports |
| `SQBRPYTHON604` | exceptions-only-exceptions | `python/` | yes | Exceptions role files may contain only custom exceptions |
| `SQBRPYTHON605` | constant-outside-constants | `python/` | yes | Public uppercase constants must be defined in the constants role |
| `SQBRPYTHON606` | banned-generic-filename | All | yes | Generic filenames hide module ownership |
| `SQBRPYTHON607` | helpers-module-name | `python/` | yes | Use an _helpers package instead of `helpers.py` |
| `SQBRPYTHON608` | classes-module-name | `python/` | yes | Use a classes package instead of `classes.py` |
| `SQBRPYTHON609` | helpers-classes-file-private | `python/` | yes | Plain classes in _helpers modules must be file-private |
| `SQBRPYTHON610` | helpers-package-layout | `python/` | yes | `_helpers/` packages must use bounded flat-or-grouped containers |
| `SQBRPYTHON611` | main-package-layout | `python/` | yes | `main/` packages must use bounded flat-or-grouped orchestration containers |
| `SQBRPYTHON612` | helpers-reserved-role-filenames | `python/` | yes | `_helpers/` packages must not contain reserved role filenames |
| `SQBRPYTHON613` | top-level-domain-shape | `python/` | yes | Top-level domains must be either role leaves or subdomain branches |
| `SQBRPYTHON614` | top-level-direct-modules | `python/` | yes | Runtime roots and top-level domains must not contain ad hoc direct modules |
| `SQBRPYTHON615` | shared-domain-prefix | `python/` | yes | Sibling domains must not encode one parent domain through a shared name prefix |
| `SQBRPYTHON616` | public-facade-import-direction | `python/` | yes | Runtime modules must not import public facade modules |
| `SQBRPYTHON617` | entry-module-shape | `python/` | yes | `main/` entry modules must expose one focused public function |
| `SQBRPYTHON618` | init-module-empty | `python/` | yes | Nested `__init__.py` files must be empty or docstring-only |
| `SQBRPYTHON619` | no-reexport-shim | `python/` | yes | Internal modules must not exist only to re-export imports |
| `SQBRPYTHON620` | no-internal-helper-exports | `python/` | yes | `_helpers/` modules must not publish an `__all__` surface |
| `SQBRPYTHON621` | main-entry-name-collision | `python/` | yes | `main/` cannot define a module and package with the same entry name |
| `SQBRPYTHON622` | public-surface-shape | `python/` | yes | Root package surfaces may contain only imports and one `__all__` declaration |
| `SQBRPYTHON623` | classes-one-class-per-module | `python/` | yes | `classes/` modules must define exactly one top-level class |
| `SQBRPYTHON624` | helpers-package-shape | `python/` | yes | `_helpers/` packages must contain no main.py orchestration entrypoints |
| `SQBRPYTHON625` | private-definition-ordering | `python/` | yes | Private constants and dataclasses must appear before top-level functions |
| `SQBRPYTHON626` | classes-runtime-reexport | `python/` | yes | `classes/` modules must not publicly re-export imported runtime symbols |
| `SQBRPYTHON627` | source-file-line-count | All | yes | Source files must stay below the configured line limit |

## Details

- Findings, exceptions, ignores and stale checks work as for every Rule; see
  [Findings and exceptions](findings-and-exceptions.md). An exception covers one
  Rule in one file.
- Findings on a file or a folder point at line 1, column 1. Columns count characters, so text before
  the finding in another script does not shift them.
- SQLBuild reads the files with the syntax of the Python it runs on. A file it cannot read fails the
  run with `cannot parse Python source` and the file's position.
- Editing one file re-checks it and the files whose findings read it. `SQBRPYTHON305` and
  `SQBRPYTHON306` read the whole project, so any Python edit re-checks them. See
  [Execution and caching](execution-and-caching.md).
