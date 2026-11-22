use crate::redaction::_helpers::patterns::{error_redacted, named_redacted};
use crate::redaction::main::redact_error_text::redact_error_text;
use crate::redaction::main::redact_masked_text::redact_masked_text;
use crate::redaction::main::redact_text::redact_text;
use crate::redaction::main::secret_field_name::secret_field_name;
use crate::redaction::tests::helpers::{
    ERROR_SHAPES, NAMED_SHAPES, SHAPE_LEADS, nul_positions, shows_value,
};
use crate::redaction::tests::test_types::{FieldNameTestCase, NulPositionTestCase, RedactTestCase};

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

#[test]
fn given_error_text_with_nul_when_redacting_then_both_readings_apply() {
    let test_cases = [
        RedactTestCase {
            description: "a match only without NUL",
            text: "pass\0word=example-fake-value refused",
            expected_text: "[REDACTED]",
        },
        RedactTestCase {
            description: "a match only as it is",
            text: "refused x\0Bearer example-fake-value",
            expected_text: "[REDACTED]",
        },
        RedactTestCase {
            description: "a cut URI only as it is",
            text: "refused 17\0postgres://orders:example-fake-value",
            expected_text: "[REDACTED]",
        },
        RedactTestCase {
            description: "no match either way",
            text: "refused \0order 17",
            expected_text: "refused order 17",
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

#[test]
fn given_masked_text_with_nul_when_redacting_then_the_mask_applies_to_both_readings() {
    let test_cases = [
        RedactTestCase {
            description: "a masked value only without NUL",
            text: "refused zebra\0quokka",
            expected_text: "[REDACTED]",
        },
        RedactTestCase {
            description: "a masked value beside NUL",
            text: "refused \0zebraquokka",
            expected_text: "[REDACTED]",
        },
        RedactTestCase {
            description: "nothing to mask",
            text: "refused \0order 17",
            expected_text: "refused order 17",
        },
        RedactTestCase {
            description: "text without NUL is only masked",
            text: "refused zebraquokka",
            expected_text: "refused ****",
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            redact_masked_text(test_case.text, |reading| reading
                .replace("zebraquokka", "****")),
            test_case.expected_text,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_field_name_with_nul_when_classified_then_nul_is_ignored() {
    let test_cases = [
        FieldNameTestCase {
            description: "NUL inside the part",
            name: "db_pass\0word",
            expected_secret: true,
        },
        FieldNameTestCase {
            description: "NUL beside the separator",
            name: "api\0-key",
            expected_secret: true,
        },
        FieldNameTestCase {
            description: "an ordinary name",
            name: "order\0_id",
            expected_secret: false,
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            secret_field_name(test_case.name),
            test_case.expected_secret,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_shape_with_nul_at_any_position_when_redacting_then_it_matches_the_stricter_reading() {
    let test_cases = [
        NulPositionTestCase {
            description: "diagnostic text",
            redact: redact_text,
            reading: named_redacted,
            shapes: &NAMED_SHAPES,
            expected_exposed: 0,
        },
        NulPositionTestCase {
            description: "error text",
            redact: redact_error_text,
            reading: error_redacted,
            shapes: &ERROR_SHAPES,
            expected_exposed: 0,
        },
    ];

    for test_case in test_cases {
        let (redact, reading) = (test_case.redact, test_case.reading);
        let mut exposed: usize = 0;
        for shape in test_case.shapes {
            assert!(
                !shows_value(&reading(shape)),
                "a shape must redact: {shape}"
            );
            assert_eq!(
                redact(shape),
                reading(shape),
                "text without NUL is unchanged"
            );
            for lead in SHAPE_LEADS {
                let whole: String = format!("{lead}{shape}");
                exposed += nul_positions(&whole)
                    .filter(|text| {
                        let hidden: bool =
                            !shows_value(&reading(text)) || !shows_value(&reading(&whole));
                        let redacted: String = redact(text);
                        (hidden && shows_value(&redacted)) || redacted.contains('\0')
                    })
                    .count();
            }
        }
        assert_eq!(
            exposed, test_case.expected_exposed,
            "{}",
            test_case.description
        );
    }
}
