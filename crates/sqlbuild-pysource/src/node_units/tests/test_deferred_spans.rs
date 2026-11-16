use crate::node_units::main::deferred_spans::deferred_spans;
use crate::node_units::tests::helpers::span;
use crate::node_units::tests::test_types::DeferredSpansTestCase;

#[test]
fn given_module_when_finding_deferred_code_then_lambda_bodies_and_the_main_block_are_spans() {
    let test_cases = [
        DeferredSpansTestCase {
            description: "a lambda body, but not its default, which runs at definition",
            contents: "H = {'x': lambda a=f(): run(a)}\n",
            expected_spans: Some(vec![span((1, 24), (1, 30))]),
        },
        DeferredSpansTestCase {
            description: "a nested lambda is inside the outer span only",
            contents: "H = lambda: (lambda: g())\n",
            expected_spans: Some(vec![span((1, 13), (1, 24))]),
        },
        DeferredSpansTestCase {
            description: "a lambda inside a function body",
            contents: "def f():\n    return sorted(r, key=lambda x: k(x))\n",
            expected_spans: Some(vec![span((2, 35), (2, 39))]),
        },
        DeferredSpansTestCase {
            description: "the main block body, either way round, but not its else clause",
            contents: "if '__main__' == __name__:\n    run()\n    stop()\nelse:\n    other(lambda: z())\n",
            expected_spans: Some(vec![span((2, 4), (3, 10)), span((5, 18), (5, 21))]),
        },
        DeferredSpansTestCase {
            description: "a main check inside a function is not the module's main block",
            contents: "def f():\n    if __name__ == '__main__':\n        run()\n",
            expected_spans: Some(Vec::new()),
        },
        DeferredSpansTestCase {
            description: "another comparison with __name__ runs on import",
            contents: "if __name__ != '__main__':\n    run()\n",
            expected_spans: Some(Vec::new()),
        },
        DeferredSpansTestCase {
            description: "a source that does not parse",
            contents: "def (:\n",
            expected_spans: None,
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            deferred_spans(test_case.contents, (3, 12)),
            test_case.expected_spans,
            "{}",
            test_case.description
        );
    }
}
