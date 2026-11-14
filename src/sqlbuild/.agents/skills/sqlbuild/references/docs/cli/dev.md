<!-- generated-by: sqlbuild skills -->

# sqb dev

> Run the server, scheduler and a reloading code location for one project on your machine.

Online: https://sqlbuild.com/docs/cli/dev/

Run the control API, the scheduler and a code location for the project in the current directory,
and open the web UI at the address it prints:

```bash
sqb dev
```

```text
Development API ready: 127.0.0.1:8000
Development location: 01a1d7ff-0b3a-7631-ba35-e44d44d072f2
Authentication disabled on loopback; triggers start paused
```

The location compiles the project and publishes it to the server. When you save a model,
orchestration file or other project input, it compiles and publishes again, so the UI and the
next run use the new version.

## Options

| Option | Default | Effect |
| --- | --- | --- |
| `--port` | `8000` | Port to listen on, on `127.0.0.1`. `0` picks a free port |
| `--target` | The configured target | Target to compile and run |
| `--control-store-url` | `.sqlbuild/control.sqlite3` in the project | Control store URL or path |
| `--no-scheduler` | Off | Serve and reload without running scheduled work |
| `--triggers` | Off | Keep each trigger's default status and your changes to it, instead of pausing every trigger at start-up |

## Run scheduled jobs locally

Triggers start paused, so a schedule never runs on your machine by accident. Resume a trigger in
the UI to let it run, or start with `--triggers` to keep the status each trigger was declared with:

```bash
sqb dev --triggers
```

## Details

- `sqb dev` listens only on loopback and needs no sign-in. Use [`sqb server`](server.md)
  for a server other people or machines connect to.
- The control store keeps runs, trigger changes and history between sessions. Delete
  `.sqlbuild/control.sqlite3` to start again from nothing.
- Stop it with Ctrl+C; it prints `Development services stopped successfully`.
