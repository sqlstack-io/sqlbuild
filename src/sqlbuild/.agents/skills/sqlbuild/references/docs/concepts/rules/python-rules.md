<!-- generated-by: sqlbuild skills -->

# Python Rules

> Check the project's Python for annotations, imports, function shape and the layout of python/nodes/.

Online: https://sqlbuild.com/docs/concepts/rules/python-rules/

## Contents

- Selecting the Python Rules
- Which files are checked
- Laying out `python/nodes/`
- Writing nodes as units
- Laying out providers
- Laying out sensors and calendars
- Testing Python
- Keeping compile-time Python pure
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
  --> sql/hooks/python/alert.py:4:11
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

The Python Rules read every supported project Python file: the runtime package under `python/nodes/` and
the folders that extend SQLBuild, such as `sql/hooks/python/`, `sql/functions/python/`, `python/providers/`,
`rules/` and `sql/macros/`. See [Project layout](../project-configuration.md#project-layout)
for the full list. Under `tests/`, only the [Python tests](#testing-python) in `tests/python/nodes/` and
`tests/python/providers/` are checked, and only by the Rules marked `tests/`.

Rules marked `python/nodes/` in the [reference](#rule-reference) check only the runtime package, because
they describe how its packages are organised. Rules marked `tests/` check only Python tests. The
others check every file except Python tests.

## Laying out `python/nodes/`

`SQBRPYTHON001` and `SQBRPYTHON002` keep `python/nodes/` organised by what the code is for, not by what
kind of node it holds. Directly under `python/nodes/`, keep only domain packages, `utils/` for code no
domain owns, and `__init__.py`:

```text
python/nodes/
  __init__.py
  fulfillment/
    shipment_export/
      asset.py         # @asset shipment_export
      main.py          # main(), the unit's implementation
  utils/
    currency/
      format.py
```

These layouts are reported:

| Finding | Example | Fix |
|---|---|---|
| A module directly under `python/nodes/` | `python/nodes/report.py` | Move it into the domain that owns it |
| A folder named for a node kind or a role | `python/nodes/loaders/`, `python/nodes/_helpers/` | Group the nodes by domain instead |
| A package with a generic name | `python/nodes/inventory/common/` | Name it after what it owns |

The generic names are `base`, `common`, `general`, `helpers`, `lib`, `misc`, `shared`, `util` and
`utils`. `utils/` is allowed directly under `python/nodes/`. A finding about a folder points at its
`__init__.py`, or at its first Python file when it has none.

## Writing nodes as units

A unit is one folder that declares one node, or one factory, and holds its implementation. Its
declaration file is named for what it declares: `loader.py`, `asset.py`, `loaders.py`, `assets.py`,
`factory.py` or `checks.py`, as compilation requires (see
[Python nodes](../python-nodes/overview.md#the-pythonnodes-folder)).
`SQBRPYTHON003` to `SQBRPYTHON012` keep each node in its own unit, grouped by domain:

```text
python/nodes/orders/
  constants.py            # shared by the orders units
  partner_export/
    asset.py              # the @asset declaration
    main.py               # main(), the implementation
    checks.py             # @check declarations for this unit
    _helpers/             # private functions for this unit
  refund_review/
    asset.py              # @asset
    main.py
```

A unit folder may also hold `models.py`, `types.py`, `constants.py`, `exceptions.py` and
`__init__.py`. The declaration only passes its arguments to `main()`:

```python
# python/nodes/orders/partner_export/asset.py
from sqlbuild.assets import AssetContext, asset
from sqlbuild.executor.python_nodes.models import PythonNodeResult

from python.providers.partner_files.provider import PartnerFiles
from python.nodes.orders.partner_export.main import main

@asset(description="Write the daily partner orders file.")
def partner_export(ctx: AssetContext, *, partner_files: PartnerFiles) -> PythonNodeResult:
    return main(ctx=ctx, partner_files=partner_files)
```

These are reported:

| Code | Finding | Fix |
|---|---|---|
| `SQBRPYTHON003` | A unit without `main.py`, or another module or folder in it | Add `main.py`; move other code into `_helpers/` or a role file |
| `SQBRPYTHON004` | A declaration file holding a helper, a constant, a class or a second declaration | Keep one `@loader`, `@asset` or `@factory` function and its imports |
| `SQBRPYTHON005` | A declaration that does more than `return main(...)`, or passes a positional argument | Move the logic into `main()` and pass every argument as `name=value` |
| `SQBRPYTHON006` | `main.py` holding more than one function, or a `main()` whose return annotation does not fit the node | Move other code into `_helpers/` and annotate `main()` with its result |
| `SQBRPYTHON007` | A signature other than `(ctx: <Kind>Context, *, provider: ProviderClass)` | Take `ctx` positionally and providers after `*`, each annotated |
| `SQBRPYTHON008` | An import of another unit's `main.py`, role files or `_helpers/`, or another domain's code | Import shared code from `python/nodes/utils/` or the domain's role files |
| `SQBRPYTHON009` | `checks.py` holding other code, or a check that does not depend on the unit's node | Name the unit's node in `depends_on` and return directly |
| `SQBRPYTHON010` | A factory that does not loop over the unit's specs, or a generated node without `name=` | Loop over specs from the unit's `constants.py` or `models.py` and name each node |
| `SQBRPYTHON011` | A docstring on a node or check, an asset or check without `description=`, or a described loader | Use `description=`; a loader takes its description from its managed source |
| `SQBRPYTHON012` | A node declared outside the unit file named for its kind, or a check outside its `checks.py` | Move it into its own unit folder, in the file named for what it declares |

`main()` returns:

| Node | Return annotation |
|---|---|
| `@asset` | `PythonNodeResult` or `PythonNodeSkipResult`, from `ctx.result(...)` and `ctx.skip(...)` |
| `@loader` | rows such as `list[...]` or `Iterator[...]`, `LoaderResult`, `LoaderSkipResult`, or `None` |

A unit may import another unit's declaration file only to name that node in `depends_on`. It may import
`provider.py`, `models.py`, `types.py`, `constants.py` and `exceptions.py` from
`python/providers/<capability>/`.

A factory's declaration file holds one `@factory` function, and a check generated by a factory may
stay in it. `@audit_factory` functions are not units and may sit anywhere under `python/nodes/`.

## Laying out providers

`SQBRPYTHON013` to `SQBRPYTHON015` keep each provider in its own capability folder, with its
private code beside it:

```text
python/providers/
  storefront_api/
    provider.py          # the one public Provider subclass
    models.py
    _classes/
      page_cursor.py     # a private collaborator class
  carrier_api/
    provider.py
```

A capability folder may also hold `types.py`, `constants.py`, `exceptions.py`, `__init__.py` and
`_helpers/`. These are reported:

| Code | Finding | Fix |
|---|---|---|
| `SQBRPYTHON013` | A module directly in `python/providers/`, a capability folder without `provider.py`, or another module or folder in it | Move it to `python/providers/<capability>/provider.py`, `_helpers/`, `_classes/` or a role file |
| `SQBRPYTHON014` | `provider.py` holding a function, a public constant, another class or a second provider | Keep one public `Provider` subclass and private constants; move the rest to `_helpers/`, `_classes/` or `constants.py` |
| `SQBRPYTHON015` | An import of a provider's `_helpers/` or `_classes/` from outside its folder, or a provider importing `python/nodes/` outside `utils/` or anything in `orchestration/` | Import the provider through `provider.py` or its role files; share code through `python/nodes/utils/` |

`SQBRPYTHON014` follows a class's bases through the project's modules, so a provider may subclass
`Provider` through an alias or a base class in `_classes/`. A class whose base comes from another
package is never reported as not being a provider.

## Laying out sensors and calendars

`SQBRPYTHON016` to `SQBRPYTHON020` keep each sensor and calendar module in a domain folder, named
for what it checks, with its private support beside it:

```text
orchestration/
  sensors/
    inventory/
      stock_feed.py      # one sensor
      _constants.py
      _helpers/
        feed_age.py
  calendars/
    finance/
      month_end.py       # one calendar
```

A domain folder's private support is `_models.py`, `_types.py`, `_constants.py`,
`_exceptions.py` and `_helpers/`. These are reported:

| Code | Finding | Fix |
|---|---|---|
| `SQBRPYTHON016` | A module directly in `sensors/` or `calendars/`, nested below its domain folder, or named `main.py`, `definitions.py`, `sensors.py`, `calendars.py` or `processing.py` | Move it to `<role>/<domain>/<capability>.py` |
| `SQBRPYTHON017` | Other private files or folders, private support outside a domain folder, or an import of another domain's private support | Use the support files or `_helpers/`; share code through `python/nodes/utils/` or a provider |
| `SQBRPYTHON018` | A sensor or calendar module holding a function, a class, a public constant or another statement besides its declaration | Move it to `_helpers/`, `_models.py` or `_constants.py` |
| `SQBRPYTHON019` | A sensor or calendar importing `python/nodes/` outside `utils/`, or code outside `orchestration/` importing it | Read node results through the sensor's context; share code through `python/nodes/utils/` or a provider |
| `SQBRPYTHON020` | A sensor or calendar function with a docstring | Keep the text in `description=` only |

Providers importing `orchestration/` are reported by `SQBRPYTHON015` instead.

## Testing Python

`sqb test` runs the pytest modules under `tests/python/nodes/`, `tests/python/providers/` and
`tests/orchestration/`; see
[Python tests](../../cli/test.md#python-tests). `SQBRPYTHON021` keeps each test beside a mirror of the
code it tests, and `SQBRPYTHON701` to `SQBRPYTHON724` keep tests table-driven:

```text
python/nodes/
  orders/
    storefront_import/
      asset.py
      _helpers/
        row_mapping.py
tests/
  python/
    nodes/
      orders/
        storefront_import/
          _helpers/
            _test_types.py      # the test-case dataclasses
            test_row_mapping.py # tests row_mapping.py
```

```python
# tests/python/nodes/orders/storefront_import/_helpers/_test_types.py
from dataclasses import dataclass

@dataclass(frozen=True)
class RowMappingTestCase:
    description: str
    status: str
    expected_label: str
```

```python
# tests/python/nodes/orders/storefront_import/_helpers/test_row_mapping.py
import pytest

from python.nodes.orders.storefront_import._helpers.row_mapping import status_label
from tests.python.nodes.orders.storefront_import._helpers._test_types import RowMappingTestCase

@pytest.mark.parametrize(
    "test_case",
    [
        RowMappingTestCase(description="shipped orders", status="S", expected_label="shipped"),
        RowMappingTestCase(description="open orders", status="O", expected_label="open"),
    ],
    ids=lambda case: case.description,
)
def test_given_a_status_when_mapping_then_returns_its_label(test_case: RowMappingTestCase) -> None:
    assert status_label(test_case.status) == test_case.expected_label
```

| Code | Finding | Fix |
|---|---|---|
| `SQBRPYTHON021` | A test module whose folder does not mirror one under `python/nodes/`, `python/providers/` or `orchestration/`, or a `test_<name>.py` with no `<name>.py` or `<name>/` package beside the code it tests | Move the test to the mirrored folder, or rename it for the module it tests |
| `SQBRPYTHON701` to `SQBRPYTHON711` | A test folder or module whose layout, imports, helpers, conditionals or test-case dataclasses break the pattern above | Follow the pattern above |
| `SQBRPYTHON712` to `SQBRPYTHON724` | A test function or `pytest.mark.parametrize` that is not named `test_given_<state>_when_<action>_then_<outcome>`, or does not take dataclass cases through `test_case` with ids from each `description` | Follow the pattern above |

## Keeping compile-time Python pure

`sqb compile` imports project Python with no warehouse connection, so the code that runs then must
not reach outside the machine. `SQBRPYTHON022` checks every call made at module scope, in a class
body, or directly in a `@factory` body, in every file it reads, and follows the project functions
those calls reach. A call that reaches a subprocess, an HTTP request, a socket, a database or
warehouse driver, or a cloud or SSH client is reported at the call where the walk started:

```python
# python/nodes/orders/storefront_import/constants.py
import requests

STOREFRONT_REGIONS = requests.get("https://example.com/regions").json()  # reported
```

```text
error[SQBRPYTHON022]: Python that runs while the project compiles must not reach a subprocess, the network, a database or a remote client: calls requests.get
  --> python/nodes/orders/storefront_import/constants.py:3:22
```

When the external call sits in a project function, the message names it and its line instead, such
as `reaches requests.get at python/nodes/orders/storefront_import/_helpers/regions.py:8`.

Calls inside node bodies, `main()`, the nodes a factory generates, and provider methods run when
the node runs, so they are not checked. Only calls whose target is known from the imports are
followed, so a call through a variable or a method of an object is never reported.

Keep the data a factory loops over in its unit's `constants.py` or `models.py`. When a factory must
discover what to generate, put that discovery behind one function and approve it. The walk does not
enter an approved function, and an approved factory's body is not checked:

```toml
[rules.rule_options.SQBRPYTHON022]
approved_loader_boundaries = [
  "python.nodes.inventory.warehouse_snapshots._helpers.spec_loading.load_warehouse_specs",
]
```

Each entry is the fully qualified name of a project function. A glob fails configuration with
`C000`, and an entry that names a module fails the run.

## Main modules and entry modules

Some Rules apply only to a domain's main modules: files under a `main/` folder, and a unit's
`main.py`, such as `python/nodes/fulfillment/shipment_export/main.py`. A main module that is not
`__init__.py` is an entry module. `SQBRPYTHON617` requires it to expose one public function, and
`SQBRPYTHON305` requires something outside its domain to import it.

`SQBRPYTHON501` to `SQBRPYTHON503` (statements, distinct calls and locals) check main modules only.
The other size limits check every function.

## Unreachable definitions

`SQBRPYTHON306` reports functions, classes and module values that nothing SQLBuild loads can reach.
SQLBuild loads every decorated function under `python/nodes/` (`@loader`, `@asset`, `@check`,
`@factory` and `@audit_factory`) and everything defined outside `python/nodes/`, such as hooks and
providers. A definition reached only through them is live. Imports through folders without an
`__init__.py` count too.

Code that SQLBuild loads in a way the Rule cannot follow, such as by name at run time, needs a rule
exception with a reason:

```toml
[[rules.rule_exceptions]]
rule = "SQBRPYTHON306"
path = "python/nodes/fulfillment/carriers/registry.py"
reason = "Carriers are looked up by name from the shipment's carrier code."
```

## Rule options

Configure the size limits and approved boundaries by exact code:

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
| `SQBRPYTHON022` | `approved_loader_boundaries` | none | Project functions the [compile-time purity](#keeping-compile-time-python-pure) walk does not enter. |

`approved_loader_boundaries` takes a list of fully qualified function names; every other option
takes a whole number of 0 or more. Unknown option names, values of the wrong type and negative
numbers fail configuration with `C000`.

## Rule reference

`Default` says whether `SQBRPYTHON` selects the Rule. `sqb rules show <code>` prints the full
remediation.

### Layout of `python/nodes/`

| Code | Name | Files | Default | Checks |
|---|---|---|---|---|
| `SQBRPYTHON001` | project-root-allowlist | `python/nodes/` | yes | `python/nodes/` may hold only domain packages, `utils/` and `__init__.py` |
| `SQBRPYTHON002` | runtime-package-owner-names | `python/nodes/` | yes | Packages under `python/nodes/` must be named for what they own |

### Node units

| Code | Name | Files | Default | Checks |
|---|---|---|---|---|
| `SQBRPYTHON003` | node-unit-layout | `python/nodes/` | yes | A unit folder holds one declaration file, `main.py`, optional role files and `_helpers/` only |
| `SQBRPYTHON004` | node-module-shape | `python/nodes/` | yes | A declaration file holds imports and exactly one node or factory declaration |
| `SQBRPYTHON005` | node-delegates-to-main | `python/nodes/` | yes | A declaration's body is `return main(...)` from its unit, with keyword arguments |
| `SQBRPYTHON006` | main-module-shape | `python/nodes/` | yes | `main.py` holds imports and one `main()` whose return annotation fits the node |
| `SQBRPYTHON007` | node-signature | `python/nodes/` | yes | `ctx` is the only positional parameter; providers are annotated and keyword-only |
| `SQBRPYTHON008` | node-import-boundaries | `python/nodes/` | yes | A unit imports only what it owns, its domain's role files, `python/nodes/utils/`, provider role files and other nodes for `depends_on` |
| `SQBRPYTHON009` | checks-module-shape | `python/nodes/` | yes | `checks.py` holds checks of the unit's node that return directly, or one factory |
| `SQBRPYTHON010` | factory-shape | `python/nodes/` | yes | A factory loops over its unit's specs and names every node it generates |
| `SQBRPYTHON011` | node-description-not-docstring | `python/nodes/` | yes | Assets and checks set `description=` without a docstring; loaders set neither |
| `SQBRPYTHON012` | node-declared-in-unit | `python/nodes/` | yes | Nodes are declared in a unit's file named for their kind, and checks in its `checks.py` |

### Providers

| Code | Name | Files | Default | Checks |
|---|---|---|---|---|
| `SQBRPYTHON013` | provider-layout | All | yes | `python/providers/` holds capability folders with `provider.py`, role files, `_helpers/` and `_classes/` only |
| `SQBRPYTHON014` | provider-module-shape | All | yes | `provider.py` holds imports, private constants and one public `Provider` subclass |
| `SQBRPYTHON015` | provider-import-boundaries | All | yes | A provider's `_helpers/` and `_classes/` are private to it; providers import only `python/nodes/utils/` from `python/nodes/` and nothing from `orchestration/` |

### Sensors and calendars

| Code | Name | Files | Default | Checks |
|---|---|---|---|---|
| `SQBRPYTHON016` | orchestration-module-paths | All | yes | Sensor and calendar modules sit at `orchestration/<role>/<domain>/<capability>.py` with a specific name |
| `SQBRPYTHON017` | orchestration-private-support | All | yes | A domain folder's private support is its support files and `_helpers/`, imported only from that folder |
| `SQBRPYTHON018` | orchestration-module-surface | All | yes | A sensor or calendar module holds imports, private constants and its one declaration |
| `SQBRPYTHON019` | orchestration-import-direction | All | yes | Sensors and calendars import only providers, `python/nodes/utils/` and their own support; nothing outside `orchestration/` imports them |
| `SQBRPYTHON020` | orchestration-no-docstring | All | yes | Sensor and calendar functions have no docstring |

### Compile-time purity

| Code | Name | Files | Default | Checks |
|---|---|---|---|---|
| `SQBRPYTHON022` | compile-time-purity | All | yes | Module scope, class bodies and `@factory` bodies reach no subprocess, network, database or remote client |

### Python tests

| Code | Name | Files | Default | Checks |
|---|---|---|---|---|
| `SQBRPYTHON021` | python-test-placement | `tests/` | yes | Python tests must mirror an existing module under `python/nodes/`, `python/providers/` or `orchestration/` |
| `SQBRPYTHON701` | test-init-module-empty | `tests/` | yes | Test package `__init__.py` files must be empty or docstring-only |
| `SQBRPYTHON702` | test-absolute-imports | `tests/` | yes | Tests must use absolute imports |
| `SQBRPYTHON703` | test-no-top-level-helpers | `tests/` | yes | Test modules may contain only tests, imports, and declarations |
| `SQBRPYTHON704` | test-no-conditionals | `tests/` | yes | Tests and local test helpers must not contain conditional control flow |
| `SQBRPYTHON705` | test-private-constant-order | `tests/` | yes | Private test constants must appear before test functions |
| `SQBRPYTHON706` | test-no-complex-comprehensions | `tests/` | yes | Nested or multi-generator comprehensions hide control flow and data shapes |
| `SQBRPYTHON707` | test-case-description | `tests/` | yes | Test-case dataclasses must define a `description` field |
| `SQBRPYTHON708` | test-case-expected-field | `tests/` | yes | Test-case dataclasses must define at least one `expected_` field |
| `SQBRPYTHON709` | test-local-types-import | `tests/` | yes | Tests must import test-case types from their local `_test_types.py` |
| `SQBRPYTHON710` | test-local-types-file | `tests/` | yes | Test directories must provide a local `_test_types.py` |
| `SQBRPYTHON711` | test-file-name | `tests/` | yes | Test modules must use a `test_` filename |
| `SQBRPYTHON712` | test-function-name | `tests/` | yes | Test functions must use `test_given_<state>_when_<action>_then_<outcome>` |
| `SQBRPYTHON713` | test-dataclass-parametrize | `tests/` | yes | Tests must use dataclass-backed pytest parameterization |
| `SQBRPYTHON714` | test-accepts-test-case | `tests/` | yes | Parametrized tests must accept a `test_case` argument |
| `SQBRPYTHON715` | test-case-annotation | `tests/` | yes | `test_case` parameters must use a local test-case dataclass annotation |
| `SQBRPYTHON716` | test-expected-field-assertion | `tests/` | yes | Tests must assert against an `expected_` field from `test_case` |
| `SQBRPYTHON717` | test-parametrize-arguments | `tests/` | yes | Pytest parametrize decorators must define parameter names and values |
| `SQBRPYTHON718` | test-parametrize-test-case | `tests/` | yes | Pytest parametrize must expose cases through the `test_case` parameter |
| `SQBRPYTHON719` | test-parametrize-ids | `tests/` | yes | Pytest parametrize decorators must define readable case ids |
| `SQBRPYTHON720` | test-inline-parametrize-values | `tests/` | yes | Pytest parametrize values must be a visible list, tuple, or local comprehension |
| `SQBRPYTHON721` | test-nonempty-parametrize-values | `tests/` | yes | Pytest parametrize case sequences must not be empty |
| `SQBRPYTHON722` | test-no-dict-cases | `tests/` | yes | Pytest cases must use typed dataclasses instead of dictionaries |
| `SQBRPYTHON723` | test-local-case-constructors | `tests/` | yes | Pytest cases must construct dataclasses from the local `_test_types.py` |
| `SQBRPYTHON724` | test-description-ids | `tests/` | yes | Pytest case ids must come from each test case `description` |

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
| `SQBRPYTHON303` | no-internal-public-surface-imports | `python/nodes/` | yes | Internal code must import from the owning module, not the bare package |
| `SQBRPYTHON304` | no-cross-domain-private-main-imports | `python/nodes/` | yes | Domain-private main entries may only be imported within their owning domain |
| `SQBRPYTHON305` | public-main-entry-external-use | `python/nodes/` | yes | Public main entries must have an importer outside their owning domain |
| `SQBRPYTHON306` | unreachable-definition | `python/nodes/` | no | Definition is unreachable from production roots |
| `SQBRPYTHON307` | no-cross-file-use-of-helper-private-class | `python/nodes/` | yes | Helper-private classes are file-local details; move shared classes to `classes/` |

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
| `SQBRPYTHON601` | models-only-models | `python/nodes/` | yes | Models role files may contain only structured runtime models |
| `SQBRPYTHON602` | types-only-types | `python/nodes/` | yes | Types role files may contain only type-layer declarations |
| `SQBRPYTHON603` | constants-only-constants | `python/nodes/` | yes | Constants role files may contain only assignments and imports |
| `SQBRPYTHON604` | exceptions-only-exceptions | `python/nodes/` | yes | Exceptions role files may contain only custom exceptions |
| `SQBRPYTHON605` | constant-outside-constants | `python/nodes/` | yes | Public uppercase constants must be defined in the constants role |
| `SQBRPYTHON606` | banned-generic-filename | All | yes | Generic filenames hide module ownership |
| `SQBRPYTHON607` | helpers-module-name | `python/nodes/` | yes | Use an _helpers package instead of `helpers.py` |
| `SQBRPYTHON608` | classes-module-name | `python/nodes/` | yes | Use a classes package instead of `classes.py` |
| `SQBRPYTHON609` | helpers-classes-file-private | `python/nodes/` | yes | Plain classes in _helpers modules must be file-private |
| `SQBRPYTHON610` | helpers-package-layout | `python/nodes/` | yes | `_helpers/` packages must use bounded flat-or-grouped containers |
| `SQBRPYTHON611` | main-package-layout | `python/nodes/` | yes | `main/` packages must use bounded flat-or-grouped orchestration containers |
| `SQBRPYTHON612` | helpers-reserved-role-filenames | `python/nodes/` | yes | `_helpers/` packages must not contain reserved role filenames |
| `SQBRPYTHON613` | top-level-domain-shape | `python/nodes/` | yes | Top-level domains must be either role leaves or subdomain branches |
| `SQBRPYTHON614` | top-level-direct-modules | `python/nodes/` | yes | Runtime roots and top-level domains must not contain ad hoc direct modules |
| `SQBRPYTHON615` | shared-domain-prefix | `python/nodes/` | yes | Sibling domains must not encode one parent domain through a shared name prefix |
| `SQBRPYTHON616` | public-facade-import-direction | `python/nodes/` | yes | Runtime modules must not import public facade modules |
| `SQBRPYTHON617` | entry-module-shape | `python/nodes/` | yes | `main/` entry modules must expose one focused public function |
| `SQBRPYTHON618` | init-module-empty | `python/nodes/` | yes | Nested `__init__.py` files must be empty or docstring-only |
| `SQBRPYTHON619` | no-reexport-shim | `python/nodes/` | yes | Internal modules must not exist only to re-export imports |
| `SQBRPYTHON620` | no-internal-helper-exports | `python/nodes/` | yes | `_helpers/` modules must not publish an `__all__` surface |
| `SQBRPYTHON621` | main-entry-name-collision | `python/nodes/` | yes | `main/` cannot define a module and package with the same entry name |
| `SQBRPYTHON622` | public-surface-shape | `python/nodes/` | yes | Root package surfaces may contain only imports and one `__all__` declaration |
| `SQBRPYTHON623` | classes-one-class-per-module | `python/nodes/` | yes | `classes/` modules must define exactly one top-level class |
| `SQBRPYTHON624` | helpers-package-shape | `python/nodes/` | yes | `_helpers/` packages must contain no main.py orchestration entrypoints |
| `SQBRPYTHON625` | private-definition-ordering | `python/nodes/` | yes | Private constants and dataclasses must appear before top-level functions |
| `SQBRPYTHON626` | classes-runtime-reexport | `python/nodes/` | yes | `classes/` modules must not publicly re-export imported runtime symbols |
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
