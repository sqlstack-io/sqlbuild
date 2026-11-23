use crate::placement::models::{DiagnosticOrderKey, PlacementIndex, PlacementUsage};
use crate::scope_index::models::{
    DeclarationIdentity, DeclarationKind, DeclarationRecord, OwnershipRoot, OwnershipRootKind,
    QualifiedIdentity, ResourceIdentity, ResourceKind, ResourceRecord, ScopeKind,
};

pub(super) fn model(name: &str) -> ResourceIdentity {
    ResourceIdentity {
        kind: ResourceKind::Model,
        name: name.to_owned(),
    }
}

fn macro_identity(name: &str) -> DeclarationIdentity {
    DeclarationIdentity {
        kind: DeclarationKind::Macro,
        name: name.to_owned(),
        owner: None,
    }
}

fn models_root() -> OwnershipRoot {
    OwnershipRoot {
        path: "models".to_owned(),
        kind: OwnershipRootKind::Resource,
        resource_kind: Some(ResourceKind::Model),
    }
}

fn model_record(name: &str, path: &str) -> ResourceRecord {
    ResourceRecord {
        identity: model(name),
        path: path.to_owned(),
        ownership_root: models_root(),
    }
}

/// A macro declared at `path` with `scope` and `owning_path` below the models root.
fn macro_record(name: &str, path: &str, placement: (ScopeKind, Option<&str>)) -> DeclarationRecord {
    let (scope, owning_path) = placement;
    DeclarationRecord {
        identity: macro_identity(name),
        path: path.to_owned(),
        line: 1,
        column: 0,
        scope,
        ownership_root: models_root(),
        owning_path: owning_path.map(str::to_owned),
        dependencies: Vec::new(),
    }
}

fn runtime(consumer: &str, declaration: &str) -> PlacementUsage {
    PlacementUsage {
        consumer: QualifiedIdentity::Resource(model(consumer)),
        declaration: macro_identity(declaration),
        through: None,
    }
}

const SALES: (ScopeKind, Option<&str>) = (ScopeKind::Local, Some("sql/models/sales"));

/// A model and a descendant-folder model, a local macro both use, and an unused global macro.
pub(super) fn descendant_models() -> PlacementIndex {
    PlacementIndex {
        resources: vec![
            model_record("orders", "sql/models/sales/orders.sql"),
            model_record("customers", "sql/models/sales/crm/customers.sql"),
        ],
        declarations: vec![
            macro_record("cents", "sql/models/sales/_macros/python/cents.py", SALES),
            macro_record(
                "unused",
                "sql/macros/python/unused.py",
                (ScopeKind::Global, None),
            ),
            macro_record("shared", "sql/models/sales/_macros/python/shared.py", SALES),
        ],
        usages: vec![
            runtime("orders", "cents"),
            runtime("orders", "shared"),
            runtime("customers", "shared"),
        ],
    }
}

/// One model using a local macro, which a further macro uses in turn.
pub(super) fn chained_macros() -> PlacementIndex {
    PlacementIndex {
        resources: vec![model_record("orders", "sql/models/sales/orders.sql")],
        declarations: vec![
            macro_record("cents", "sql/models/sales/_macros/python/cents.py", SALES),
            macro_record("round", "sql/models/sales/_macros/python/round.py", SALES),
        ],
        usages: vec![
            runtime("orders", "cents"),
            PlacementUsage {
                consumer: QualifiedIdentity::Declaration(macro_identity("cents")),
                declaration: macro_identity("round"),
                through: None,
            },
        ],
    }
}

const SALES_GROUPED: (ScopeKind, Option<&str>) = (ScopeKind::Local, Some("sql/models/sales"));
const SALES_SHARED: (ScopeKind, Option<&str>) = (ScopeKind::Inherited, Some("sql/models/sales"));

/// Two sales models sharing a local `cents`, beside an optional descendant-public `labels`.
fn shared_cents(cents_path: &str, labels_path: Option<&str>) -> PlacementIndex {
    let mut declarations = vec![macro_record("cents", cents_path, SALES_GROUPED)];
    declarations.extend(labels_path.map(|path| macro_record("labels", path, SALES_SHARED)));
    PlacementIndex {
        resources: vec![
            model_record("orders", "sql/models/sales/orders.sql"),
            model_record("refunds", "sql/models/sales/refunds.sql"),
            model_record("customers", "sql/models/sales/crm/customers.sql"),
        ],
        declarations,
        usages: vec![
            runtime("orders", "cents"),
            runtime("refunds", "cents"),
            runtime("refunds", "labels"),
            runtime("customers", "labels"),
        ],
    }
}

/// A grouped `cents` and an empty descendant-public role.
pub(super) fn grouped_cents_empty_role() -> PlacementIndex {
    shared_cents("sql/models/sales/_sqlbuild/_macros/python/money/cents.py", None)
}

/// A grouped `cents` and a descendant-public role keeping its files in groups.
pub(super) fn grouped_cents_grouped_role() -> PlacementIndex {
    shared_cents(
        "sql/models/sales/_sqlbuild/_macros/python/money/cents.py",
        Some("sql/models/sales/_sqlbuild/macros/python/text/labels.py"),
    )
}

/// A direct `cents` and a descendant-public role keeping its files in groups.
pub(super) fn direct_cents_grouped_role() -> PlacementIndex {
    shared_cents(
        "sql/models/sales/_sqlbuild/_macros/python/cents.py",
        Some("sql/models/sales/_sqlbuild/macros/python/text/labels.py"),
    )
}

/// A grouped `cents` and a descendant-public role keeping its files directly.
pub(super) fn grouped_cents_direct_role() -> PlacementIndex {
    shared_cents(
        "sql/models/sales/_sqlbuild/_macros/python/money/cents.py",
        Some("sql/models/sales/_sqlbuild/macros/python/labels.py"),
    )
}

/// Owned `(model, destination)` moves.
pub(super) fn owned_moves(moves: &[(&str, &str)]) -> Vec<(ResourceIdentity, String)> {
    moves
        .iter()
        .map(|(name, destination)| (model(name), (*destination).to_owned()))
        .collect()
}

/// Owned `(declaration, path, owning path)` rows of expected relocations.
pub(super) fn owned_relocations(
    items: &[(usize, &str, Option<&str>)],
) -> Vec<(usize, String, Option<String>)> {
    items
        .iter()
        .map(|(position, path, owning)| (*position, (*path).to_owned(), owning.map(str::to_owned)))
        .collect()
}

fn order_key(
    path: Option<&str>,
    line: Option<i64>,
    code: &str,
    message: &str,
) -> DiagnosticOrderKey {
    DiagnosticOrderKey {
        path: path.map(str::to_owned),
        line,
        column: None,
        code: code.to_owned(),
        message: message.to_owned(),
    }
}

/// Diagnostics without a path or line first, then by path, line, code and message.
pub(super) fn mixed_diagnostics() -> Vec<DiagnosticOrderKey> {
    vec![
        order_key(Some("sql/models/b.sql"), Some(2), "S010", "b"),
        order_key(Some("sql/models/a.sql"), Some(9), "S010", "a"),
        order_key(None, None, "S019", "parse"),
        order_key(Some(""), Some(0), "S019", "parse"),
        order_key(Some("sql/models/a.sql"), Some(9), "S007", "z"),
        order_key(Some("sql/models/b.sql"), None, "S010", "b"),
    ]
}
