"""Stable native layering policy constants."""

LAYERS_METADATA_KEY: str = "native-layers"
PYO3_PACKAGE: str = "pyo3"
POLYGLOT_PACKAGE: str = "polyglot-sql"
DEPENDENCY_TABLES: tuple[str, ...] = ("dependencies", "build-dependencies", "dev-dependencies")
CARGO_MANIFEST: str = "Cargo.toml"
CARGO_LOCKFILE: str = "Cargo.lock"
PYPROJECT_MANIFEST: str = "pyproject.toml"
PYPROJECT_LOCKFILE: str = "uv.lock"
LOCK_SOURCE_KEY: str = "source"
PYPROJECT_LOCK_SOURCE: dict[str, str] = {"editable": "."}
RELEASE_MANIFEST: str = ".release-please-manifest.json"
RELEASE_MANIFEST_PACKAGE: str = "."
CLI_DISTRIBUTION_PIN_PREFIX: str = "sqlbuild-cli=="
CRATES_DIRECTORY: str = "crates"
RELEASE_CRATES: tuple[str, ...] = ("sqlbuild-cli", "sqlbuild-python")
PACKAGE_VERSION_VARIABLE: str = "CARGO_PKG_VERSION"
