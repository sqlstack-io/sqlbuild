use crate::scope_cache::models::{CachedDeclaration, CachedIndex, EnumFacts};
use crate::scope_index::models::{
    DeclarationIdentity, DeclarationKind, DeclarationRecord, OwnershipRoot, OwnershipRootKind,
    ResourceIdentity, ResourceKind, ResourceRecord, ScopeKind,
};

fn models_root() -> OwnershipRoot {
    OwnershipRoot {
        path: "models".to_owned(),
        kind: OwnershipRootKind::Resource,
        resource_kind: Some(ResourceKind::Model),
    }
}

/// A complete index with one model and one enum declaration.
pub(super) fn orders_index() -> CachedIndex {
    CachedIndex {
        roots: vec![models_root()],
        resources: vec![ResourceRecord {
            identity: ResourceIdentity {
                kind: ResourceKind::Model,
                name: "orders".to_owned(),
            },
            path: "sql/models/sales/orders.sql".to_owned(),
            ownership_root: models_root(),
        }],
        declarations: vec![CachedDeclaration {
            record: DeclarationRecord {
                identity: DeclarationIdentity {
                    kind: DeclarationKind::Enum,
                    name: "order_status".to_owned(),
                    owner: None,
                },
                path: "sql/models/sales/enums/status.yml".to_owned(),
                line: 1,
                column: 1,
                scope: ScopeKind::Inherited,
                ownership_root: models_root(),
                owning_path: Some("sql/models/sales".to_owned()),
                dependencies: Vec::new(),
            },
            macro_facts: None,
            enum_facts: Some(EnumFacts {
                members: vec!["placed".to_owned(), "shipped".to_owned()],
                scalar_type: "VARCHAR".to_owned(),
            }),
            constant_facts: None,
        }],
        completeness: [true; 6],
        ..CachedIndex::default()
    }
}

pub(super) fn unchanged(bytes: Vec<u8>) -> Vec<u8> {
    bytes
}

/// The envelope with its payload digest's first hex digit replaced by one that differs.
pub(super) fn wrong_digest(bytes: Vec<u8>) -> Vec<u8> {
    let text: String = String::from_utf8(bytes).expect("ascii envelope");
    let marker: &str = "\"payload_sha256\":\"";
    let start: usize = text.find(marker).expect("digest field") + marker.len();
    let digit: char = text[start..].chars().next().expect("digest digit");
    let replacement: &str = ["0", "1"][usize::from(digit == '0')];
    format!("{}{replacement}{}", &text[..start], &text[start + 1..]).into_bytes()
}

pub(super) fn truncated(bytes: Vec<u8>) -> Vec<u8> {
    bytes[..bytes.len() / 2].to_vec()
}
