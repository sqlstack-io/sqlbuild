use crate::scope_index::main::parse_identity::parse_identity;
use crate::scope_index::models::{
    DeclarationIdentity, DeclarationKind, IdentityFailure, QualifiedIdentity, ResourceIdentity,
    ResourceKind,
};
use crate::scope_index::tests::test_types::ParseIdentityTestCase;

#[test]
fn given_qualified_value_when_parsing_then_python_identity_or_failure_is_returned() {
    let test_cases = [
        ParseIdentityTestCase {
            description: "resource kinds win over declaration kinds",
            value: "model:orders",
            expected_identity: Ok(QualifiedIdentity::Resource(ResourceIdentity {
                kind: ResourceKind::Model,
                name: "orders".to_owned(),
            })),
        },
        ParseIdentityTestCase {
            description: "a public declaration",
            value: "enum:order_status",
            expected_identity: Ok(QualifiedIdentity::Declaration(DeclarationIdentity {
                kind: DeclarationKind::Enum,
                name: "order_status".to_owned(),
                owner: None,
            })),
        },
        ParseIdentityTestCase {
            description: "a private declaration splits its owner at the last dot",
            value: "constant:model:orders.v2.limit",
            expected_identity: Ok(QualifiedIdentity::Declaration(DeclarationIdentity {
                kind: DeclarationKind::Constant,
                name: "limit".to_owned(),
                owner: Some(ResourceIdentity {
                    kind: ResourceKind::Model,
                    name: "orders.v2".to_owned(),
                }),
            })),
        },
        ParseIdentityTestCase {
            description: "a path folder is an unknown kind",
            value: "sql/models/a:b/orders.sql",
            expected_identity: Err(IdentityFailure::UnknownKind("sql/models/a".to_owned())),
        },
        ParseIdentityTestCase {
            description: "an empty name",
            value: "model:",
            expected_identity: Err(IdentityFailure::NoName),
        },
        ParseIdentityTestCase {
            description: "a private identity without an owner name",
            value: "enum:model:.status",
            expected_identity: Err(IdentityFailure::InvalidPrivate),
        },
        ParseIdentityTestCase {
            description: "too many parts",
            value: "a:b:c:d",
            expected_identity: Err(IdentityFailure::Unqualified),
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            parse_identity(test_case.value),
            test_case.expected_identity,
            "{}",
            test_case.description
        );
    }
}
