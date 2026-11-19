use crate::fingerprint_identity::main::canonical_text::canonical_text;
use crate::fingerprint_identity::main::decoded_object::decoded_object;
use crate::fingerprint_identity::main::model_local_identity_hash::model_local_identity_hash;
use crate::fingerprint_identity::main::recorded_model_local_hash::recorded_model_local_hash;
use crate::fingerprint_identity::tests::test_types::RecordedLocalHashTestCase;

#[test]
fn given_recorded_metadata_when_hashing_then_it_matches_the_config_as_written() {
    let test_cases = [
        RecordedLocalHashTestCase {
            description: "a written-out default refresh mode stays in the hash",
            metadata_json: r#"{"config":{"materialized":"maintained","refresh_mode":"incremental"},"execution_signature":{},"local_function_hashes":{},"model_name":"daily_totals"}"#,
            expected_hashed_json: r#"{"config":{"materialized":"maintained","refresh_mode":"incremental"},"execution_signature":{},"local_function_hashes":{},"model_name":"daily_totals"}"#,
        },
        RecordedLocalHashTestCase {
            description: "a model that declares no refresh mode",
            metadata_json: r#"{"config":{"materialized":"maintained"},"execution_signature":{},"local_function_hashes":{},"model_name":"daily_totals"}"#,
            expected_hashed_json: r#"{"config":{"materialized":"maintained"},"execution_signature":{},"local_function_hashes":{},"model_name":"daily_totals"}"#,
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            recorded_model_local_hash("query_hash", test_case.metadata_json),
            Some(model_local_identity_hash(
                "query_hash",
                &canonical_text(
                    decoded_object(test_case.expected_hashed_json)
                        .expect("metadata is JSON")
                        .expect("metadata is an object")
                )
            )),
            "{}",
            test_case.description
        );
    }
}
