<!-- generated-by: sqlbuild skills -->

# Providers

> Shared runtime services for Python nodes and hooks.

Online: https://sqlbuild.com/docs/concepts/python-nodes/providers/

## Contents

- Defining a provider
- Provider name
- Using providers in Python nodes
- Using providers in hooks
- Using providers via context
- Lifecycle
- Configuration from environment variables
- Discovery rules
- Plan output
- Providers in the web UI
- Project layout

Providers are shared runtime services that SQLBuild discovers, configures, and injects into your Python nodes and hooks. Use them for external connections, API clients, or any stateful service that multiple nodes need access to.

## Defining a provider

Create a Python file under `providers/` in your project. A provider is a class that subclasses `Provider` from `sqlbuild.providers`:

```python
# providers/warehouse_client.py
from sqlbuild.providers import Provider

class WarehouseClient(Provider):
    """Client for the reporting warehouse."""
    api_key: str
    endpoint: str = "https://api.example.com"

    def setup(self, ctx):
        self.session = create_session(self.api_key, self.endpoint)

    def teardown(self):
        self.session.close()
```

The class docstring is the provider's description and is required; a provider without one fails compilation with `P010` ([Descriptions](../models/configuration.md#descriptions)).

`Provider` extends [pydantic-settings `BaseSettings`](https://docs.pydantic.dev/latest/concepts/pydantic_settings/), so provider fields are validated and can be populated from environment variables automatically.

## Provider name

Each provider has a runtime name used for injection. By default, the name is derived from the class name by converting to `lower_snake_case`:

- `WarehouseClient` becomes `warehouse_client`
- `SlackNotifier` becomes `slack_notifier`

Override the name explicitly with `provider_name`:

```python
class WarehouseClient(Provider):
    """Client for the reporting warehouse."""
    provider_name = "warehouse"
    api_key: str
```

Provider names must be valid Python identifiers (`lower_snake_case`). They share the global project resource namespace with models, sources, seeds, functions, loaders, tasks, assets, checks, and hooks.

## Using providers in Python nodes

Providers are injected into Python node functions by **parameter name**. Add a parameter whose name matches the provider's runtime name:

```python
from sqlbuild.tasks import task

@task
def export_orders(ctx, warehouse_client):
    """Export orders."""
    warehouse_client.session.upload(ctx.query("SELECT * FROM orders"))
```

SQLBuild matches the parameter name `warehouse_client` to the discovered provider with that name, sets it up if it hasn't been already, and passes it to the function.

You can also type-annotate the parameter for IDE support and compile-time validation:

```python
from sqlbuild.tasks import task
from providers.warehouse_client import WarehouseClient

@task
def export_orders(ctx, warehouse_client: WarehouseClient):
    """Export orders."""
    warehouse_client.session.upload(ctx.query("SELECT * FROM orders"))
```

When a type annotation is present, SQLBuild validates that the discovered provider is an instance of the annotated class. A mismatch raises a compile-time error.

Provider injection works in all Python node types:

- **Loaders** (`@loader`)
- **Tasks** (`@task`)
- **Assets** (`@asset`)
- **Checks** (`@check`)

## Using providers in hooks

Python lifecycle hooks also support provider injection by parameter name:

```python
# hooks/python/notify.py
from sqlbuild.hooks import hook

@hook
def notify_complete(ctx, slack_notifier):
    """Notify complete."""
    slack_notifier.send(f"Model {ctx.model_name} built successfully")
```

```sql
MODEL (
  description "One row per order",
  materialized table,
  post_hooks [python("notify_complete")],
);
```

Providers are also available on the `HookContext` via `ctx.providers`:

```python
@hook
def notify_complete(ctx):
    """Notify complete."""
    notifier = ctx.providers.slack_notifier
    notifier.send(f"Model {ctx.model_name} built successfully")
```

## Using providers via context

All Python node contexts (`TaskContext`, `AssetContext`, `CheckContext`, `LoaderContext`) and `HookContext` expose a `ctx.providers` container for name-based access:

```python
@task
def export_orders(ctx):
    """Export orders."""
    client = ctx.providers.warehouse_client
    client.session.upload(ctx.query("SELECT * FROM orders"))
```

Both approaches (parameter injection and `ctx.providers`) are equivalent. Parameter injection is more explicit and enables type checking; `ctx.providers` is useful when provider access is conditional or dynamic.

## Lifecycle

Providers follow a lazy setup, reverse-teardown lifecycle scoped to the command invocation:

1. **Discovery** - on compile, SQLBuild discovers all `Provider` subclasses under `providers/` and validates their settings (from environment variables or field defaults).
2. **Lazy setup** - `setup(ctx)` is called the first time a provider is accessed during a build, not at startup. Providers that are never used are never set up.
3. **Teardown** - after the command completes, `teardown()` is called on all providers that were set up, in reverse setup order. Teardown runs even if the build failed.

```python
class WarehouseClient(Provider):
    """Client for the reporting warehouse."""
    api_key: str

    def setup(self, ctx):
        # Called once, the first time any node accesses this provider
        self.connection = connect(self.api_key)

    def teardown(self):
        # Called after the command completes
        self.connection.close()
```

Both `setup` and `teardown` are optional. A provider with only field declarations and no lifecycle methods is valid; it acts as a validated configuration object.

## Configuration from environment variables

Because `Provider` extends `pydantic-settings BaseSettings`, fields without defaults are read from environment variables. The environment variable name matches the field name in uppercase:

```python
class SlackNotifier(Provider):
    """Posts build notifications to a chat channel."""
    slack_token: str          # reads SLACK_TOKEN from environment
    channel: str = "#builds"  # has a default, environment variable is optional
```

See the [pydantic-settings documentation](https://docs.pydantic.dev/latest/concepts/pydantic_settings/) for advanced configuration like custom env prefixes, `.env` file support, and nested settings.

## Discovery rules

- Provider classes are discovered from `.py` files under `providers/` recursively
- Files named `__init__.py` or starting with `_` are skipped
- Each concrete (non-abstract) subclass of `Provider` is registered
- Provider names must be globally unique across project resources
- Settings are validated at discovery time. Missing required fields (without environment variables set) raise a discovery error immediately, not at runtime

## Plan output

When providers are used by Python nodes or hooks, `sqb plan` shows a Providers section listing each provider and its consumers:

```
Providers
  warehouse_client  2 nodes
  slack_notifier    1 node
```

Use `--verbose` to see which specific nodes consume each provider.

## Providers in the web UI

When a location publishes to [`sqb server`](../../cli/server.md), the **Providers** page lists each
provider with its class and docstring. A provider's page shows:

| Tab | Shows |
| --- | --- |
| Settings | Each setting's type, environment variable and whether it is required; secret settings are marked |
| Used by | The loaders, tasks, hooks and other code that take the provider, and the jobs that run that code |

Setting values, defaults included, are never published.

## Project layout

```
my-project/
  providers/
    warehouse_client.py
    slack_notifier.py
  hooks/
    sql/
      record_notification.sql
    python/
      notify.py
  python/
    loaders/
      load_orders.py
    tasks/
      export_orders.py
```
