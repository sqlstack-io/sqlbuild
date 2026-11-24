use crate::compiled_project::errors::DeclarationError;
use crate::compiled_project::main::declaration_error_message::declaration_error_message;
use crate::compiled_project::models::InaccessibleDeclaration;
use crate::compiled_project::tests::test_types::DeclarationErrorTestCase;

#[test]
fn given_declaration_error_when_wording_then_attachment_message_is_returned() {
    let test_cases = [
        DeclarationErrorTestCase {
            description: "unknown private enum lists visible enums sorted",
            error: DeclarationError::Unknown {
                kind: "enum".to_owned(),
                name: "_status".to_owned(),
                visible: vec!["region".to_owned(), "channel".to_owned()],
            },
            expected_message: "Unknown enum '_status' in this model in 'sql/models/orders.sql'. \
                               Visible enums: channel, region",
        },
        DeclarationErrorTestCase {
            description: "unknown constant with none visible",
            error: DeclarationError::Unknown {
                kind: "constant".to_owned(),
                name: "cap".to_owned(),
                visible: Vec::new(),
            },
            expected_message: "Unknown constant 'cap' in 'sql/models/orders.sql'. Visible constants: none",
        },
        DeclarationErrorTestCase {
            description: "inaccessible resource-owned constant",
            error: DeclarationError::Inaccessible {
                kind: "constant".to_owned(),
                name: "cap".to_owned(),
                record: InaccessibleDeclaration {
                    path: "sql/models/marts/_constants/limits.sql".to_owned(),
                    line: 1,
                    column: 11,
                    scope: "directory".to_owned(),
                    owner: "sql/models/marts".to_owned(),
                    owner_identity: Some(("model".to_owned(), "orders_daily".to_owned())),
                },
            },
            expected_message: "Constant 'cap' is known but inaccessible in 'sql/models/orders.sql'. \
                               It is defined at sql/models/marts/_constants/limits.sql:1:11 with \
                               directory scope owned by 'sql/models/marts' (model 'orders_daily'); \
                               consumer path: 'sql/models/orders.sql'",
        },
        DeclarationErrorTestCase {
            description: "invalid enum reference stop",
            error: DeclarationError::ScanStop(2),
            expected_message: "Invalid enum reference in 'sql/models/orders.sql'",
        },
        DeclarationErrorTestCase {
            description: "unclosed quote stop",
            error: DeclarationError::ScanStop(0),
            expected_message: "Enum and constant expansion contains an unclosed quoted string",
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            declaration_error_message(&test_case.error, "sql/models/orders.sql"),
            test_case.expected_message,
            "{}",
            test_case.description
        );
    }
}
