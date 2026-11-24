use crate::scope_index::main::report_lookup::report_lookup;
use crate::scope_index::models::{
    DeclarationIdentity, DeclarationKind, DeclarationRecord, OwnershipRoot, OwnershipRootKind,
    QualifiedIdentity, ReportLookup, ReportRecords, ReportUsage, ResourceIdentity, ResourceKind,
    ResourceRecord, ScopeKind,
};
use crate::scope_report::models::{
    DeclarationValues, MacroValues, QueryTarget, ReportFilters, ReportValues,
};

/// One model under `sql/models/sales`, a global macro, an inherited constant and a local enum.
pub(super) fn orders_facts() -> (ReportRecords, ReportLookup, ReportValues) {
    let models: OwnershipRoot = OwnershipRoot {
        path: "sql/models".to_owned(),
        kind: OwnershipRootKind::Resource,
        resource_kind: Some(ResourceKind::Model),
    };
    let project: OwnershipRoot = OwnershipRoot {
        path: ".".to_owned(),
        kind: OwnershipRootKind::Global,
        resource_kind: None,
    };
    let orders: ResourceIdentity = ResourceIdentity {
        kind: ResourceKind::Model,
        name: "orders".to_owned(),
    };
    let declaration =
        |kind, name: &str, path: &str, scope, root: &OwnershipRoot, owner: Option<&str>| {
            DeclarationRecord {
                identity: DeclarationIdentity {
                    kind,
                    name: name.to_owned(),
                    owner: None,
                },
                path: path.to_owned(),
                line: 1,
                column: 1,
                scope,
                ownership_root: root.clone(),
                owning_path: owner.map(str::to_owned),
                dependencies: Vec::new(),
            }
        };
    let declarations: Vec<DeclarationRecord> = vec![
        declaration(
            DeclarationKind::Macro,
            "g",
            "sql/macros/g.py",
            ScopeKind::Global,
            &project,
            None,
        ),
        declaration(
            DeclarationKind::Constant,
            "c",
            "sql/models/constants/c.sql",
            ScopeKind::Inherited,
            &models,
            Some("sql/models"),
        ),
        declaration(
            DeclarationKind::Enum,
            "e",
            "sql/models/finance/_enums/e.sql",
            ScopeKind::Local,
            &models,
            Some("sql/models/finance"),
        ),
    ];
    let records: ReportRecords = ReportRecords {
        roots: vec![models.clone()],
        resources: vec![ResourceRecord {
            identity: orders.clone(),
            path: "sql/models/sales/orders.sql".to_owned(),
            ownership_root: models,
        }],
        usages: vec![ReportUsage {
            consumer: QualifiedIdentity::Resource(orders),
            declaration: declarations[1].identity.clone(),
            kind: "runtime".to_owned(),
            through: None,
        }],
        declarations,
        ..ReportRecords::default()
    };
    let lookup: ReportLookup = report_lookup(&records).expect("valid paths");
    let values: ReportValues = ReportValues {
        declarations: vec![
            DeclarationValues {
                macro_values: Some(MacroValues {
                    parameters: vec!["x".to_owned()],
                    ..MacroValues::default()
                }),
                ..DeclarationValues::default()
            },
            DeclarationValues::default(),
            DeclarationValues::default(),
        ],
        ..ReportValues::default()
    };
    (records, lookup, values)
}

/// Every section, nearby declarations included, and every global listed.
pub(super) fn open_filters() -> ReportFilters {
    ReportFilters {
        sections: vec![
            "available".to_owned(),
            "used".to_owned(),
            "relationship_scope".to_owned(),
        ],
        include_nearby: true,
        defined_under: None,
        kinds: Vec::new(),
        glob: None,
        used_only: false,
        dependency_depth: 0,
        sort: "identity".to_owned(),
        cursor: None,
        page_size: 100,
        nearby_depth: 1,
        globals: "all".to_owned(),
    }
}

pub(super) fn text_target(target: Option<&str>) -> Option<QueryTarget> {
    target.map(|text| QueryTarget::Text(text.to_owned()))
}
