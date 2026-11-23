use crate::macro_modules::tests::helpers::{expected_rows, inventory_rows};
use crate::macro_modules::tests::test_types::{
    InventoryMacroModulesTestCase, NativeParseErrorTestCase,
};

const SHARED: (&str, &str) = (
    "sql/macros/b.py",
    "def b():\n    pass\n\n\ndef c():\n    pass\n",
);

#[test]
fn given_macro_modules_when_inventoried_then_python_analysis_agrees() {
    let test_cases = [
        InventoryMacroModulesTestCase {
            description: "a clean project lists exports, signatures, dependencies and load order",
            sources: &[
                (
                    "sql/macros/shared.py",
                    "def shared(value, /, other, *rest, key, **options):\n    return value\n",
                ),
                (
                    "sql/macros/orders.py",
                    "from .shared import shared as render\n\n\ndef _helper():\n    return \
                     render(1)\n\n\n@decorated\nasync \\\n  def orders():\n    return _helper()\n",
                ),
            ],
            expected_exports: &[
                ("orders", 1, &[], 9, &["shared"]),
                (
                    "shared",
                    0,
                    &["value", "other", "rest", "key", "options"],
                    1,
                    &[],
                ),
            ],
            expected_faults: &[],
            expected_modules: Some(&[
                (0, "macros.shared", &[]),
                (1, "macros.orders", &["macros.shared"]),
            ]),
        },
        InventoryMacroModulesTestCase {
            description: "module-level uses report the first name in breadth-first order",
            sources: &[
                SHARED,
                (
                    "sql/macros/a.py",
                    "from sql.macros.b import b, c\n_Y = [[c], b]\n\n\ndef a():\n    return 1\n",
                ),
            ],
            expected_exports: &[("b", 0, &[], 1, &[]), ("c", 0, &[], 5, &[])],
            expected_faults: &[(
                1,
                "Macro module 'sql/macros/a.py' uses imported macro 'b' outside an ordinary function \
                 body; call imported macros directly from module-level macro or helper functions",
            )],
            expected_modules: None,
        },
        InventoryMacroModulesTestCase {
            description: "elif branches nest as CPython's orelse",
            sources: &[
                SHARED,
                (
                    "sql/macros/a.py",
                    "from sql.macros.b import b, c\nif x:\n    pass\nelif c:\n    pass\nelse:\n    \
                     b\n\n\ndef a():\n    return 1\n",
                ),
            ],
            expected_exports: &[("b", 0, &[], 1, &[]), ("c", 0, &[], 5, &[])],
            expected_faults: &[(
                1,
                "Macro module 'sql/macros/a.py' uses imported macro 'c' outside an ordinary function \
                 body; call imported macros directly from module-level macro or helper functions",
            )],
            expected_modules: None,
        },
        InventoryMacroModulesTestCase {
            description: "function bodies report the first dynamic use in depth-first order",
            sources: &[
                SHARED,
                (
                    "sql/macros/a.py",
                    "from sql.macros.b import b, c\n\n\ndef a():\n    return f(b, [c])\n",
                ),
            ],
            expected_exports: &[("b", 0, &[], 1, &[]), ("c", 0, &[], 5, &[])],
            expected_faults: &[(
                1,
                "Macro module 'sql/macros/a.py' uses imported macro 'b' dynamically; call imported \
                 macros directly",
            )],
            expected_modules: None,
        },
        InventoryMacroModulesTestCase {
            description: "a faulty module is dropped and the rest are still inventoried",
            sources: &[
                ("sql/macros/bad.py", "PUBLIC = 1\n"),
                ("sql/macros/good.py", "def good():\n    return 1\n"),
            ],
            expected_exports: &[("good", 1, &[], 1, &[])],
            expected_faults: &[(
                0,
                "Macro module 'sql/macros/bad.py' declaration 'PUBLIC' must be underscore-private",
            )],
            expected_modules: None,
        },
        InventoryMacroModulesTestCase {
            description: "an import cycle faults, then its importer fails once the cycle is gone",
            sources: &[
                (
                    "sql/macros/a.py",
                    "from sql.macros.b import b\n\n\ndef a():\n    return b()\n",
                ),
                (
                    "sql/macros/b.py",
                    "from sql.macros.a import a\n\n\ndef b():\n    return a()\n",
                ),
            ],
            expected_exports: &[],
            expected_faults: &[
                (
                    0,
                    "Macro import cycle in 'sql/macros/a.py': macros.a -> macros.b -> macros.a",
                ),
                (
                    1,
                    "Macro module 'sql/macros/b.py' imports project package 'macros.a' instead of a \
                     macro module",
                ),
            ],
            expected_modules: None,
        },
        InventoryMacroModulesTestCase {
            description: "a local call cycle reachable from an export faults",
            sources: &[(
                "sql/macros/a.py",
                "def _h():\n    return a()\n\n\ndef a():\n    return _h()\n",
            )],
            expected_exports: &[],
            expected_faults: &[(0, "Macro call cycle in 'sql/macros/a.py': a -> _h -> a")],
            expected_modules: None,
        },
        InventoryMacroModulesTestCase {
            description: "modules sharing a dotted name each fault",
            sources: &[
                ("sql/macros/a/b.py", "def first():\n    pass\n"),
                ("sql/macros/a.b.py", "def second():\n    pass\n"),
                ("sql/macros/c.py", "def third():\n    pass\n"),
            ],
            expected_exports: &[("third", 2, &[], 1, &[])],
            expected_faults: &[
                (
                    0,
                    "Macro module path collision for 'macros.a.b': sql/macros/a/b.py and sql/macros/a.b.py",
                ),
                (
                    1,
                    "Macro module path collision for 'macros.a.b': sql/macros/a/b.py and sql/macros/a.b.py",
                ),
            ],
            expected_modules: None,
        },
        InventoryMacroModulesTestCase {
            description: "repeated export names fault without blocking the load plan",
            sources: &[
                ("sql/macros/a.py", "def same():\n    pass\n"),
                ("sql/macros/b.py", "def same():\n    pass\n"),
                ("sql/macros/c.py", "def same():\n    pass\n"),
            ],
            expected_exports: &[],
            expected_faults: &[
                (
                    1,
                    "Macro name collision for 'same': sql/macros/a.py and sql/macros/b.py",
                ),
                (2, "Macro name collision for 'same' at sql/macros/c.py"),
            ],
            expected_modules: Some(&[
                (0, "macros.a", &[]),
                (1, "macros.b", &[]),
                (2, "macros.c", &[]),
            ]),
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            inventory_rows(test_case.sources),
            Ok(expected_rows(
                test_case.expected_exports,
                test_case.expected_faults,
                test_case.expected_modules,
            )),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_source_ruff_rejects_when_inventoried_then_native_error_names_module() {
    let test_cases = [NativeParseErrorTestCase {
        description: "a source CPython was said to accept but ruff rejects is a native error",
        sources: &[("sql/macros/a.py", "def a(:\n")],
        expected_message_prefix: "Native macro module analysis cannot parse 'sql/macros/a.py', which \
                                  CPython accepted: ",
    }];
    for test_case in test_cases {
        assert!(
            inventory_rows(test_case.sources)
                .is_err_and(|message| message.starts_with(test_case.expected_message_prefix)),
            "{}",
            test_case.description
        );
    }
}
