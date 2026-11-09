use crate::redaction::main::redact_error_text::redact_error_text;
use crate::redaction::main::redact_text::redact_text;
use crate::redaction::tests::test_types::RedactTestCase;

#[test]
fn given_diagnostic_text_when_redacting_then_matches_python_substitutions() {
    let test_cases = [
        RedactTestCase {
            description: "authorization header keeps its scheme separator",
            text: "Authorization: Bearer abc.def",
            expected_text: "Authorization: [REDACTED]",
        },
        RedactTestCase {
            description: "named secrets and URI passwords",
            text: "password = 'hunter2', url=postgres://user:pw@host/db",
            expected_text: "password = [REDACTED], url=postgres://user:[REDACTED]@host/db",
        },
        RedactTestCase {
            description: "plain text is unchanged",
            text: "nothing to see",
            expected_text: "nothing to see",
        },
        RedactTestCase {
            description: "a key glued to a prefix with a separator",
            text: "db_password=orders-pass host=warehouse",
            expected_text: "db_password=[REDACTED] host=warehouse",
        },
        RedactTestCase {
            description: "a key glued to a prefix without a separator",
            text: "PGPASSWORD=orders-pass psql",
            expected_text: "PGPASSWORD=[REDACTED] psql",
        },
        RedactTestCase {
            description: "near miss: a key that only starts with a secret word keeps its value",
            text: "password_policy=strict max_tokens=5 tokenizer: words passwords: 3 tries",
            expected_text: "password_policy=strict max_tokens=5 tokenizer: words passwords: 3 tries",
        },
        RedactTestCase {
            description: "near miss: pwd stays a whole word, so working directories are kept",
            text: "OLDPWD=/srv/orders cwd=/srv/orders pwd=orders-pass",
            expected_text: "OLDPWD=/srv/orders cwd=/srv/orders pwd=[REDACTED]",
        },
        RedactTestCase {
            description: "a quoted PASSWORD in DDL",
            text: "CREATE ROLE orders_app WITH LOGIN PASSWORD 'orders-pass' VALID UNTIL 'infinity'",
            expected_text: "CREATE ROLE orders_app WITH LOGIN PASSWORD [REDACTED] VALID UNTIL 'infinity'",
        },
        RedactTestCase {
            description: "IDENTIFIED BY with a quoted and a bare password",
            text: "CREATE USER orders IDENTIFIED BY 'orders-pass'; ALTER USER stock IDENTIFIED BY stockpass",
            expected_text: "CREATE USER orders IDENTIFIED BY [REDACTED]; ALTER USER stock IDENTIFIED BY [REDACTED]",
        },
        RedactTestCase {
            description: "IDENTIFIED WITH a plugin BY a password, as ClickHouse writes it",
            text: "CREATE USER orders IDENTIFIED WITH sha256_password BY 'orders-pass' HOST ANY",
            expected_text: "CREATE USER orders IDENTIFIED WITH sha256_password BY [REDACTED] HOST ANY",
        },
        RedactTestCase {
            description: "near miss: prose about passwords and IDENTIFIED WITH keep their words",
            text: "password authentication failed for user \"orders\"; IDENTIFIED WITH caching_sha2_password",
            expected_text: "password authentication failed for user \"orders\"; IDENTIFIED WITH caching_sha2_password",
        },
        RedactTestCase {
            description: "an unencoded @ inside a URI password",
            text: "url=postgres://orders:p@ss@warehouse.example.test/orders",
            expected_text: "url=postgres://orders:[REDACTED]@warehouse.example.test/orders",
        },
        RedactTestCase {
            description: "near miss: an @ after the host belongs to the path or query, not the password",
            text: "postgres://orders:pw@warehouse.example.test/orders?contact=support@example.com, https://api.example.test/v1",
            expected_text: "postgres://orders:[REDACTED]@warehouse.example.test/orders?contact=support@example.com, https://api.example.test/v1",
        },
        RedactTestCase {
            description: "two URIs in a list each lose their password",
            text: "postgres://orders:one@h1.example.test,postgres://stock:two@h2.example.test",
            expected_text: "postgres://orders:[REDACTED]@h1.example.test,postgres://stock:[REDACTED]@h2.example.test",
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            redact_text(test_case.text),
            test_case.expected_text,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_error_text_with_cut_uris_when_redacting_then_position_tells_a_port_from_a_password() {
    let test_cases = [
        RedactTestCase {
            description: "an all-digit password in a URI cut before its host",
            text: "could not parse 'postgres://orders:1234",
            expected_text: "could not parse 'postgres://orders:[REDACTED]",
        },
        RedactTestCase {
            description: "a password cut at a closing quote",
            text: "dsn 'postgres://orders:orders-pass' rejected",
            expected_text: "dsn 'postgres://orders:[REDACTED]' rejected",
        },
        RedactTestCase {
            description: "a cut password containing / is not a port",
            text: "cut 'postgres://orders:pa/ss",
            expected_text: "cut 'postgres://orders:[REDACTED]/ss",
        },
        RedactTestCase {
            description: "near miss: a port the authority ends after stays",
            text: "GET http://warehouse.example.test:8080/health failed; retry http://warehouse.example.test:8443?attempt=2",
            expected_text: "GET http://warehouse.example.test:8080/health failed; retry http://warehouse.example.test:8443?attempt=2",
        },
        RedactTestCase {
            description: "a complete URI is redacted once and a second pass changes nothing",
            text: "postgres://orders:[REDACTED]@warehouse.example.test:5432/orders",
            expected_text: "postgres://orders:[REDACTED]@warehouse.example.test:5432/orders",
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            redact_error_text(test_case.text),
            test_case.expected_text,
            "{}",
            test_case.description
        );
    }
}
