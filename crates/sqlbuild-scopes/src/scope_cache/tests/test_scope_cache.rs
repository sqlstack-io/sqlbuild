use crate::scope_cache::_helpers::decode::decode_envelope;
use crate::scope_cache::_helpers::fingerprint::config_bytes;
use crate::scope_cache::main::encode_scope_cache::encode_scope_cache;
use crate::scope_cache::models::{CacheMiss, CachedIndex};
use crate::scope_cache::tests::helpers::{orders_index, truncated, unchanged, wrong_digest};
use crate::scope_cache::tests::test_types::{ConfigBytesTestCase, ReadBackTestCase};

#[test]
fn given_written_envelope_when_reading_back_then_only_an_intact_matching_cache_hits() {
    let test_cases = [
        ReadBackTestCase {
            description: "an intact envelope for the same fingerprint decodes the same index",
            read_fingerprint: "inputs",
            altered: unchanged,
            expected_read: Ok(()),
        },
        ReadBackTestCase {
            description: "another fingerprint is stale",
            read_fingerprint: "other",
            altered: unchanged,
            expected_read: Err(CacheMiss::Stale),
        },
        ReadBackTestCase {
            description: "a payload digest mismatch is stale",
            read_fingerprint: "inputs",
            altered: wrong_digest,
            expected_read: Err(CacheMiss::Stale),
        },
        ReadBackTestCase {
            description: "truncated bytes are unreadable",
            read_fingerprint: "inputs",
            altered: truncated,
            expected_read: Err(CacheMiss::Unreadable),
        },
    ];
    for test_case in test_cases {
        let envelope: Vec<u8> = encode_scope_cache(&orders_index(), "inputs").expect("encodes");

        let read: Result<CachedIndex, CacheMiss> =
            decode_envelope(&(test_case.altered)(envelope), test_case.read_fingerprint);

        assert_eq!(
            read.map(|index| index == orders_index()),
            test_case.expected_read.map(|()| true),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_config_file_when_fingerprinting_then_only_scope_relevant_keys_are_hashed() {
    let test_cases = [
        ConfigBytesTestCase {
            description: "keys outside the scope set and target fields beyond vars drop",
            name: "sqlbuild_project.toml",
            text: "name = \"orders\"\n[vars]\nregion = \"eu\"\n[targets.dev]\nschema = \"x\"\n[targets.dev.vars]\nday = 1\n",
            expected_bytes: "{\"targets\":{\"dev\":{\"vars\":{\"day\":1}}},\"vars\":{\"region\":\"eu\"}}",
        },
        ConfigBytesTestCase {
            description: "a non-TOML config file hashes its raw bytes",
            name: ".sqlbuildignore",
            text: "sql/models/tmp\n",
            expected_bytes: "sql/models/tmp\n",
        },
        ConfigBytesTestCase {
            description: "invalid TOML hashes its raw bytes",
            name: "sqlbuild_local.toml",
            text: "adapter = \n",
            expected_bytes: "adapter = \n",
        },
    ];
    for test_case in test_cases {
        let bytes: Vec<u8> = config_bytes(test_case.name, test_case.text.as_bytes())
            .unwrap_or_else(|_| panic!("{} is serializable", test_case.description));

        assert_eq!(
            String::from_utf8(bytes).expect("utf-8"),
            test_case.expected_bytes,
            "{}",
            test_case.description
        );
    }
}
