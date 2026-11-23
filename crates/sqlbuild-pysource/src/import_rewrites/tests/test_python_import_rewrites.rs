use crate::import_rewrites::main::python_import_rewrites::python_import_rewrites;
use crate::import_rewrites::models::{ImportEdit, ImportRewrites, ModuleMove, PythonSource};
use crate::import_rewrites::tests::helpers::applied;
use crate::import_rewrites::tests::test_types::ImportRewriteTestCase;

const CENTS: &str = "sql/models/staging/_sqlbuild/_macros/cents.py";
const MOVED_CENTS: &str = "sql/models/finance/_sqlbuild/_macros/cents.py";
const CENTS_MOVE: &[(&str, &str)] = &[(CENTS, MOVED_CENTS)];

#[test]
fn given_moved_modules_when_rewriting_imports_then_each_import_follows_or_is_refused() {
    let test_cases = [
        ImportRewriteTestCase {
            description: "a direct from-import follows the module",
            sources: &[
                (CENTS, "def to_cents(x):\n    return x\n"),
                (
                    "python/nodes/checks/amounts.py",
                    "from sql.models.staging._sqlbuild._macros.cents import to_cents\n",
                ),
            ],
            moves: CENTS_MOVE,
            expected_sources: &[(
                "python/nodes/checks/amounts.py",
                "from sql.models.finance._sqlbuild._macros.cents import to_cents\n",
            )],
            expected_refusals: &[],
        },
        ImportRewriteTestCase {
            description: "an aliased module import and a grouped import follow the module",
            sources: &[(
                "python/nodes/checks/amounts.py",
                "import sql.models.staging._sqlbuild._macros.cents as cents_module\n\
                 from sql.models.staging._sqlbuild._macros.cents import (\n    to_cents,\n    \
                 to_dollars,\n)\n\n\ndef check():\n    import sql.models.staging._sqlbuild._macros.cents \
                 as inner\n    return inner\n",
            )],
            moves: CENTS_MOVE,
            expected_sources: &[(
                "python/nodes/checks/amounts.py",
                "import sql.models.finance._sqlbuild._macros.cents as cents_module\n\
                 from sql.models.finance._sqlbuild._macros.cents import (\n    to_cents,\n    \
                 to_dollars,\n)\n\n\ndef check():\n    import sql.models.finance._sqlbuild._macros.cents \
                 as inner\n    return inner\n",
            )],
            expected_refusals: &[],
        },
        ImportRewriteTestCase {
            description: "a module imported by name from its package moves the package",
            sources: &[(
                "python/nodes/checks/amounts.py",
                "from sql.models.staging._sqlbuild._macros import cents\n",
            )],
            moves: CENTS_MOVE,
            expected_sources: &[(
                "python/nodes/checks/amounts.py",
                "from sql.models.finance._sqlbuild._macros import cents\n",
            )],
            expected_refusals: &[],
        },
        ImportRewriteTestCase {
            description: "relative imports that would climb far become absolute",
            sources: &[
                (
                    CENTS,
                    "from .rates import RATE\nfrom . import rounding\n\n\n\
                     def to_cents(x):\n    return x * RATE\n",
                ),
                ("sql/models/staging/_sqlbuild/_macros/rates.py", "RATE = 100\n"),
                (
                    "sql/models/staging/_sqlbuild/_macros/totals.py",
                    "from .cents import to_cents\n",
                ),
            ],
            moves: CENTS_MOVE,
            expected_sources: &[
                (
                    CENTS,
                    "from sql.models.staging._sqlbuild._macros.rates import RATE\n\
                     from sql.models.staging._sqlbuild._macros import rounding\n\n\n\
                     def to_cents(x):\n    return x * RATE\n",
                ),
                (
                    "sql/models/staging/_sqlbuild/_macros/totals.py",
                    "from sql.models.finance._sqlbuild._macros.cents import to_cents\n",
                ),
            ],
            expected_refusals: &[],
        },
        ImportRewriteTestCase {
            description: "a sibling move keeps a short relative import",
            sources: &[(
                "sql/macros/money/totals.py",
                "from ..money.cents import to_cents\nfrom .cents import to_dollars\n",
            )],
            moves: &[("sql/macros/money/cents.py", "sql/macros/text/cents.py")],
            expected_sources: &[(
                "sql/macros/money/totals.py",
                "from ..text.cents import to_cents\nfrom ..text.cents import to_dollars\n",
            )],
            expected_refusals: &[],
        },
        ImportRewriteTestCase {
            description: "imports of modules that stay are left alone",
            sources: &[(
                "python/nodes/checks/amounts.py",
                "import os\nfrom models.staging._sqlbuild._macros.rates import RATE\n",
            )],
            moves: CENTS_MOVE,
            expected_sources: &[],
            expected_refusals: &[],
        },
        ImportRewriteTestCase {
            description: "an import without an alias is refused",
            sources: &[(
                "python/nodes/checks/amounts.py",
                "import os\nimport models.staging._sqlbuild._macros.cents\n",
            )],
            moves: CENTS_MOVE,
            expected_sources: &[],
            expected_refusals: &[(
                "python/nodes/checks/amounts.py:2:8",
                "imports models.staging._sqlbuild._macros.cents without an alias",
            )],
        },
        ImportRewriteTestCase {
            description: "a dynamic import naming the module is refused",
            sources: &[(
                "python/nodes/checks/amounts.py",
                "import importlib\n\nCENTS = importlib.import_module(\
                 \"models.staging._sqlbuild._macros.cents\")\n",
            )],
            moves: CENTS_MOVE,
            expected_sources: &[],
            expected_refusals: &[(
                "python/nodes/checks/amounts.py:3:9",
                "imports models.staging._sqlbuild._macros.cents dynamically by name",
            )],
        },
        ImportRewriteTestCase {
            description: "a star import from the moved module's package is refused",
            sources: &[(
                "python/nodes/checks/amounts.py",
                "from sql.models.staging._sqlbuild._macros import *\n",
            )],
            moves: CENTS_MOVE,
            expected_sources: &[],
            expected_refusals: &[(
                "python/nodes/checks/amounts.py:1:1",
                "star-imports the package of models.staging._sqlbuild._macros.cents",
            )],
        },
        ImportRewriteTestCase {
            description: "a star import from the moved module itself follows it",
            sources: &[(
                "python/nodes/checks/amounts.py",
                "from sql.models.staging._sqlbuild._macros.cents import *\n",
            )],
            moves: CENTS_MOVE,
            expected_sources: &[(
                "python/nodes/checks/amounts.py",
                "from sql.models.finance._sqlbuild._macros.cents import *\n",
            )],
            expected_refusals: &[],
        },
        ImportRewriteTestCase {
            description: "a moved module imported beside a name that stays is refused",
            sources: &[(
                "python/nodes/checks/amounts.py",
                "from sql.models.staging._sqlbuild._macros import cents, rates\n",
            )],
            moves: CENTS_MOVE,
            expected_sources: &[],
            expected_refusals: &[(
                "python/nodes/checks/amounts.py:1:1",
                "imports a moved module from sql.models.staging._sqlbuild._macros together with names",
            )],
        },
        ImportRewriteTestCase {
            description: "a moved module whose package binds its name is refused",
            sources: &[
                (
                    "sql/models/staging/_sqlbuild/_macros/__init__.py",
                    "from .cents import cents\n",
                ),
                (
                    "python/nodes/checks/amounts.py",
                    "import os\nfrom models.staging._sqlbuild._macros import cents\n",
                ),
            ],
            moves: CENTS_MOVE,
            expected_sources: &[(
                "sql/models/staging/_sqlbuild/_macros/__init__.py",
                "from sql.models.finance._sqlbuild._macros.cents import cents\n",
            )],
            expected_refusals: &[(
                "python/nodes/checks/amounts.py:2:1",
                "imports cents from sql.models.staging._sqlbuild._macros, whose __init__.py also \
                     binds cents",
            )],
        },
        ImportRewriteTestCase {
            description: "a module imported by a name the move changes is refused",
            sources: &[(
                "python/nodes/checks/amounts.py",
                "from sql.models.staging._sqlbuild._macros import cents\n",
            )],
            moves: &[(CENTS, "sql/models/finance/_sqlbuild/_macros/money.py")],
            expected_sources: &[],
            expected_refusals: &[(
                "python/nodes/checks/amounts.py:1:1",
                "imports the moved module models.staging._sqlbuild._macros.cents by its name, \
                 which becomes models.finance._sqlbuild._macros.money",
            )],
        },
    ];
    for test_case in test_cases {
        let sources: Vec<PythonSource> = test_case
            .sources
            .iter()
            .map(|(path, contents)| PythonSource {
                path: (*path).to_owned(),
                contents: (*contents).to_owned(),
            })
            .collect();
        let moves: Vec<ModuleMove> = test_case
            .moves
            .iter()
            .map(|(from, to)| ModuleMove {
                from: (*from).to_owned(),
                to: (*to).to_owned(),
            })
            .collect();

        let rewrites: ImportRewrites = python_import_rewrites(&sources, &moves);

        let rewritten: Vec<(String, String)> = sources
            .iter()
            .filter_map(|source| {
                let edits: Vec<&ImportEdit> = rewrites
                    .edits
                    .iter()
                    .filter(|edit| edit.path == source.path)
                    .collect();
                (!edits.is_empty())
                    .then(|| (source.path.clone(), applied(&source.contents, &edits)))
            })
            .collect();
        let refusals: Vec<(String, bool)> = rewrites
            .refusals
            .iter()
            .zip(test_case.expected_refusals)
            .map(|(refusal, (_, reason))| {
                (
                    format!("{}:{}:{}", refusal.path, refusal.line, refusal.column),
                    refusal.reason.starts_with(reason),
                )
            })
            .collect();
        assert_eq!(
            rewritten,
            test_case
                .expected_sources
                .iter()
                .map(|(path, contents)| ((*path).to_owned(), (*contents).to_owned()))
                .collect::<Vec<_>>(),
            "{}",
            test_case.description
        );
        assert_eq!(
            (rewrites.refusals.len(), refusals),
            (
                test_case.expected_refusals.len(),
                test_case
                    .expected_refusals
                    .iter()
                    .map(|(location, _)| ((*location).to_owned(), true))
                    .collect::<Vec<_>>()
            ),
            "{}: {:?}",
            test_case.description,
            rewrites.refusals
        );
    }
}
