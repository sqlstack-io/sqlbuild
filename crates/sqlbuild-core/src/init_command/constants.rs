//! The directories a blank project starts with.

/// Each resource directory, in creation order.
pub const SCAFFOLD_DIRECTORIES: [&str; 16] = [
    "sql/models/staging",
    "sql/models/marts",
    "sql/schemas",
    "sql/sources",
    "sql/seeds",
    "python/nodes",
    "sql/hooks/sql",
    "sql/hooks/python",
    "tests/unit",
    "tests/scenarios",
    "sql/functions/sql",
    "sql/functions/python",
    "sql/macros/sql",
    "sql/macros/python",
    "sql/audits/generic",
    "sql/audits/singular",
];

/// The placeholder file kept in otherwise empty directories.
pub const GITKEEP_FILE: &str = ".gitkeep";

/// The numbered next steps before the compile and build commands.
pub const NEXT_STEPS: [&str; 6] = [
    "  1. Add sources to sql/sources/",
    "  2. Add seeds to sql/seeds/",
    "  3. Add Python loaders, assets, checks, and factories to python/nodes/",
    "  4. Add SQL hooks to sql/hooks/sql/, Python hooks to sql/hooks/python/, functions to sql/functions/, or macros to sql/macros/sql/ or sql/macros/python/",
    "  5. Add models to sql/models/staging/ and sql/models/marts/",
    "  6. Add tests to tests/unit/ or tests/scenarios/, reusable audits to sql/audits/generic/, or standalone audits to sql/audits/singular/",
];

/// The git ignore file `sqb init` and the playground add the env file to.
pub(crate) const GITIGNORE_FILE: &str = ".gitignore";
/// The lines that keep the project env file, which holds provider secrets, out of git.
pub(crate) const ENV_FILE_IGNORE: &str =
    "# Provider settings and secrets that sqb loads (env_file); never commit them\n.env\n";
