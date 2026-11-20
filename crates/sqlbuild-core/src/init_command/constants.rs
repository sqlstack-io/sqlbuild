//! The directories a blank project starts with.

/// Each resource directory, in creation order.
pub const SCAFFOLD_DIRECTORIES: [&str; 16] = [
    "models/staging",
    "models/marts",
    "schemas",
    "sources",
    "seeds",
    "python",
    "hooks/sql",
    "hooks/python",
    "tests/unit",
    "tests/scenarios",
    "functions/sql",
    "functions/python",
    "macros/sql",
    "macros/python",
    "audits/generic",
    "audits/singular",
];

/// The placeholder file kept in otherwise empty directories.
pub const GITKEEP_FILE: &str = ".gitkeep";

/// The numbered next steps before the compile and build commands.
pub const NEXT_STEPS: [&str; 6] = [
    "  1. Add sources to sources/",
    "  2. Add seeds to seeds/",
    "  3. Add Python loaders, tasks, assets, checks, and factories to python/",
    "  4. Add SQL hooks to hooks/sql/, Python hooks to hooks/python/, functions to functions/, or macros to macros/sql/ or macros/python/",
    "  5. Add models to models/staging/ and models/marts/",
    "  6. Add tests to tests/unit/ or tests/scenarios/, reusable audits to audits/generic/, or standalone audits to audits/singular/",
];

/// The git ignore file `sqb init` and the playground add the env file to.
pub(crate) const GITIGNORE_FILE: &str = ".gitignore";
/// The lines that keep the project env file, which holds provider secrets, out of git.
pub(crate) const ENV_FILE_IGNORE: &str =
    "# Provider settings and secrets that sqb loads (env_file); never commit them\n.env\n";
