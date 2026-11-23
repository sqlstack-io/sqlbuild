use crate::diff_selection::main::resolve_diff_selection::resolve_diff_selection;
use crate::diff_selection::tests::helpers::{orders_graph, owned};
use crate::diff_selection::tests::test_types::DiffSelectionTestCase;

#[test]
fn given_selectors_when_resolving_diff_selection_then_matches_python_semantics() {
    let test_cases = [
        DiffSelectionTestCase {
            description: "graph operators expand both ways",
            select: &["+orders+"],
            exclude: &[],
            expected_selection: Ok(&[0, 1, 2]),
        },
        DiffSelectionTestCase {
            description: "whitespace separates tokens and exclude removes",
            select: &["orders\u{3000}customers"],
            exclude: &["customers"],
            expected_selection: Ok(&[0]),
        },
        DiffSelectionTestCase {
            description: "a comma intersects tag and path",
            select: &["tag:finance,path:sql\\models\\marts\\"],
            exclude: &[],
            expected_selection: Ok(&[2, 3]),
        },
        DiffSelectionTestCase {
            description: "a nested path selects only below it",
            select: &["marts/finance+"],
            exclude: &[],
            expected_selection: Ok(&[2]),
        },
        DiffSelectionTestCase {
            description: "an operator-only token is empty",
            select: &["++"],
            exclude: &[],
            expected_selection: Err("S001"),
        },
        DiffSelectionTestCase {
            description: "an unknown name",
            select: &["orders,payments"],
            exclude: &[],
            expected_selection: Err("S007"),
        },
        DiffSelectionTestCase {
            description: "an unknown tag",
            select: &["tag:ops"],
            exclude: &[],
            expected_selection: Err("S008"),
        },
        DiffSelectionTestCase {
            description: "an unknown path",
            select: &["path:sql/models/intermediate"],
            exclude: &[],
            expected_selection: Err("S009"),
        },
    ];

    for test_case in test_cases {
        let resolved = resolve_diff_selection(
            &orders_graph(),
            (&owned(test_case.select), &owned(test_case.exclude)),
        );
        let mut selection: Vec<usize> = resolved.clone().unwrap_or_default().into_iter().collect();
        selection.sort_unstable();

        assert_eq!(
            resolved
                .as_ref()
                .map(|_| selection.as_slice())
                .map_err(|error| error.code),
            test_case.expected_selection,
            "{}",
            test_case.description
        );
    }
}
