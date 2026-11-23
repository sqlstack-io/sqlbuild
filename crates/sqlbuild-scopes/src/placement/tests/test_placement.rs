use crate::placement::_helpers::declaration_directory::declaration_directory;
use crate::placement::main::diagnostic_order::diagnostic_order;
use crate::placement::main::placement_diagnostics::placement_diagnostics;
use crate::placement::main::relocate_for_resource_moves::relocate_for_resource_moves;
use crate::placement::models::Relocation;
use crate::placement::tests::helpers::{
    chained_macros, descendant_models, direct_cents_grouped_role, grouped_cents_direct_role,
    grouped_cents_empty_role, grouped_cents_grouped_role, mixed_diagnostics, model, owned_moves,
    owned_relocations,
};
use crate::placement::tests::test_types::{
    CarriedLayoutTestCase, DeclarationDirectoryTestCase, DiagnosticOrderTestCase,
    PlacementDiagnosticsTestCase, RelocationTestCase,
};
use crate::scope_index::models::{DeclarationKind, ScopeKind};

#[test]
fn given_usages_when_validating_placement_then_unused_and_misplaced_declarations_are_reported() {
    let test_cases = [
        PlacementDiagnosticsTestCase {
            description: "an unused global and a local macro needed by a descendant folder",
            index: descendant_models,
            expected_diagnostics: &[
                (
                    1,
                    "S010",
                    "Unused global declaration 'macro:unused' at sql/macros/python/unused.py; remove it or \
                     add a genuine runtime use",
                ),
                (
                    2,
                    "S007",
                    "Declaration 'macro:shared' is currently exact-owner-private at \
                     'sql/models/sales' (sql/models/sales/_macros/python/shared.py); required descendant-public \
                     at 'sql/models/sales'. Consumers: model:customers, model:orders. Move it to \
                     'sql/models/sales/_sqlbuild/macros/python/'",
                ),
            ],
        },
        PlacementDiagnosticsTestCase {
            description: "a macro used only through another macro inherits its anchors",
            index: chained_macros,
            expected_diagnostics: &[],
        },
    ];
    for test_case in test_cases {
        let diagnostics: Vec<(usize, &str, String)> = placement_diagnostics(&(test_case.index)())
            .into_iter()
            .map(|item| (item.declaration, item.code.as_str(), item.message))
            .collect();

        assert_eq!(
            diagnostics,
            test_case
                .expected_diagnostics
                .iter()
                .map(|(position, code, message)| (*position, *code, (*message).to_owned()))
                .collect::<Vec<_>>(),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_placement_when_choosing_directory_then_role_folders_match_python() {
    let test_cases = [
        DeclarationDirectoryTestCase {
            description: "a global macro",
            kind: DeclarationKind::Macro,
            scope: ScopeKind::Global,
            path: Some("sql/models/sales"),
            expected_directory: "macros",
        },
        DeclarationDirectoryTestCase {
            description: "a local audit below an authored root",
            kind: DeclarationKind::Audit,
            scope: ScopeKind::Local,
            path: Some("models"),
            expected_directory: "sql/models/_audits/generic",
        },
        DeclarationDirectoryTestCase {
            description: "an inherited hook below a nested folder",
            kind: DeclarationKind::SqlHook,
            scope: ScopeKind::Inherited,
            path: Some("sql/models/sales"),
            expected_directory: "sql/models/sales/_sqlbuild/hooks/sql",
        },
        DeclarationDirectoryTestCase {
            description: "an inherited enum without a folder",
            kind: DeclarationKind::Enum,
            scope: ScopeKind::Inherited,
            path: None,
            expected_directory: "enums",
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            declaration_directory(test_case.kind, test_case.scope, test_case.path),
            test_case.expected_directory,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_resource_move_when_relocating_then_chained_local_declarations_follow() {
    let test_cases = [
        RelocationTestCase {
            description: "a model and the macro chain it uses move to a new folder",
            index: chained_macros,
            destination: "sql/models/finance/orders.sql",
            expected_relocations: Some(&[
                (
                    0,
                    "sql/models/finance/_sqlbuild/_macros/python/cents.py",
                    Some("sql/models/finance"),
                ),
                (
                    1,
                    "sql/models/finance/_sqlbuild/_macros/python/round.py",
                    Some("sql/models/finance"),
                ),
            ]),
        },
        RelocationTestCase {
            description: "a move within the same folder relocates nothing",
            index: chained_macros,
            destination: "sql/models/sales/renamed.sql",
            expected_relocations: Some(&[]),
        },
    ];
    for test_case in test_cases {
        let relocations: Option<Vec<Relocation>> = relocate_for_resource_moves(
            &(test_case.index)(),
            &[(model("orders"), test_case.destination.to_owned())],
        );

        assert_eq!(
            relocations.map(|items| {
                items
                    .into_iter()
                    .map(|item| (item.declaration, item.path, item.owning_path))
                    .collect::<Vec<_>>()
            }),
            test_case.expected_relocations.map(owned_relocations),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_destination_role_layout_when_carrying_declaration_then_its_shape_is_matched() {
    let test_cases = [
        CarriedLayoutTestCase {
            description: "an empty destination role keeps the source group",
            index: grouped_cents_empty_role,
            moves: &[("orders", "sql/models/sales/eu/orders.sql")],
            expected_relocations: &[(
                0,
                "sql/models/sales/_sqlbuild/macros/python/money/cents.py",
                false,
            )],
        },
        CarriedLayoutTestCase {
            description: "a grouped destination role keeps the source group",
            index: grouped_cents_grouped_role,
            moves: &[("orders", "sql/models/sales/eu/orders.sql")],
            expected_relocations: &[(
                0,
                "sql/models/sales/_sqlbuild/macros/python/money/cents.py",
                false,
            )],
        },
        CarriedLayoutTestCase {
            description: "a direct source in a grouped destination role needs a group",
            index: direct_cents_grouped_role,
            moves: &[("orders", "sql/models/sales/eu/orders.sql")],
            expected_relocations: &[(0, "sql/models/sales/_sqlbuild/macros/python/cents.py", true)],
        },
        CarriedLayoutTestCase {
            description: "a direct destination role drops the source group",
            index: grouped_cents_direct_role,
            moves: &[("orders", "sql/models/sales/eu/orders.sql")],
            expected_relocations: &[(
                0,
                "sql/models/sales/_sqlbuild/macros/python/cents.py",
                false,
            )],
        },
        CarriedLayoutTestCase {
            description: "moving every user together places the macro once at their new folder",
            index: grouped_cents_grouped_role,
            moves: &[
                ("orders", "sql/models/sales/eu/orders.sql"),
                ("refunds", "sql/models/sales/eu/refunds.sql"),
            ],
            expected_relocations: &[(
                0,
                "sql/models/sales/eu/_sqlbuild/_macros/python/money/cents.py",
                false,
            )],
        },
    ];
    for test_case in test_cases {
        let relocations: Option<Vec<Relocation>> =
            relocate_for_resource_moves(&(test_case.index)(), &owned_moves(test_case.moves));

        assert_eq!(
            relocations.map(|items| {
                items
                    .into_iter()
                    .map(|item| (item.declaration, item.path, item.requires_group))
                    .collect::<Vec<_>>()
            }),
            Some(
                test_case
                    .expected_relocations
                    .iter()
                    .map(|(position, path, group)| (*position, (*path).to_owned(), *group))
                    .collect()
            ),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_diagnostics_when_ordering_then_python_sort_key_and_stability_hold() {
    let test_cases = [DiagnosticOrderTestCase {
        description: "missing paths and lines sort as empty and zero; equal keys keep their order",
        keys: mixed_diagnostics,
        expected_order: &[2, 3, 4, 1, 5, 0],
    }];
    for test_case in test_cases {
        assert_eq!(
            diagnostic_order(&(test_case.keys)()),
            test_case.expected_order,
            "{}",
            test_case.description
        );
    }
}
