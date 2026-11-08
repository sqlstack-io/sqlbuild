//! Migration fingerprint cache keys and entries stay byte-identical to the stored format.

use crate::planner_warehouse::main::fingerprint_cache_entry::fingerprint_cache_entry;
use crate::planner_warehouse::main::fingerprint_cache_key::fingerprint_cache_key;
use crate::planner_warehouse::main::fingerprint_cache_lookup::fingerprint_cache_lookup;
use crate::planner_warehouse::main::fingerprint_cache_shared_digest::fingerprint_cache_shared_digest;
use crate::planner_warehouse::models::FingerprintCacheKey;
use crate::planner_warehouse::tests::test_types::{
    FingerprintDigestTestCase, FingerprintLookupTestCase,
};

#[test]
fn given_key_parts_when_hashing_then_matches_stored_digests() {
    let test_cases = [FingerprintDigestTestCase {
        description: "python cache digests",
        key: FingerprintCacheKey {
            shared_digest: "shared".to_owned(),
            query_sql: "SELECT 1".to_owned(),
            metadata_json: "{}".to_owned(),
            dialect: Some("duckdb".to_owned()),
            visible_renames: vec![("orders".to_owned(), "abc".to_owned())],
        },
        expected_key: "835490d6bfb10f11b9649eb56324524a30685ef55634d585940417179b4e28ad",
        expected_shared: "5f90bf52c3459f6c880d8307b8e528cb97766198a73310cd20681ca1b06fe15f",
        expected_entry: "391ffa0fdaf437fff6e7f6d1f0fc01e5509751580667cc37f101ba020c707eb6\n{\"f\":\"fp1\",\"k\":\"k1\",\"v\":1}",
    }];
    for test_case in test_cases {
        assert_eq!(
            fingerprint_cache_key(&test_case.key).unwrap_or_default(),
            test_case.expected_key,
            "{}",
            test_case.description
        );
        assert_eq!(
            fingerprint_cache_shared_digest("1.0").unwrap_or_default(),
            test_case.expected_shared,
            "{}",
            test_case.description
        );
        assert_eq!(
            fingerprint_cache_entry("k1", Some("fp1")).unwrap_or_default(),
            test_case.expected_entry,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_stored_entry_when_looking_up_then_accepts_only_intact_entries_for_the_key() {
    let test_cases = [
        FingerprintLookupTestCase {
            description: "intact fingerprint",
            stored_fingerprint: Some("fp1"),
            tamper: None,
            lookup_key: "k1",
            expected_lookup: Some(Some("fp1")),
        },
        FingerprintLookupTestCase {
            description: "intact absent fingerprint",
            stored_fingerprint: None,
            tamper: None,
            lookup_key: "k1",
            expected_lookup: Some(None),
        },
        FingerprintLookupTestCase {
            description: "other key",
            stored_fingerprint: Some("fp1"),
            tamper: None,
            lookup_key: "k2",
            expected_lookup: None,
        },
        FingerprintLookupTestCase {
            description: "edited payload",
            stored_fingerprint: Some("fp1"),
            tamper: Some(("fp1", "fp2")),
            lookup_key: "k1",
            expected_lookup: None,
        },
        FingerprintLookupTestCase {
            description: "missing separator",
            stored_fingerprint: Some("fp1"),
            tamper: Some(("\n", "")),
            lookup_key: "k1",
            expected_lookup: None,
        },
    ];
    for test_case in test_cases {
        let entry = fingerprint_cache_entry("k1", test_case.stored_fingerprint).unwrap_or_default();
        let stored = test_case
            .tamper
            .map_or(entry.clone(), |(from, to)| entry.replace(from, to));
        assert_eq!(
            fingerprint_cache_lookup(&stored, test_case.lookup_key),
            test_case
                .expected_lookup
                .map(|found| found.map(str::to_owned)),
            "{}",
            test_case.description
        );
    }
}
