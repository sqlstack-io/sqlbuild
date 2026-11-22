//! A read keeps the check list it built only when no signal named its target while it read.

use axum::body::Bytes;

use crate::node_states::_helpers::checks::{drop_checks, held_checks, keep_checks};
use crate::node_states::models::{CachedJson, HeldCheckList, TargetCache};
use crate::node_states::tests::test_types::HeldChecksTestCase;

#[test]
fn given_a_check_list_read_when_signals_arrive_before_it_keeps_its_list_then_only_an_unsignalled_read_keeps_it()
 {
    let test_cases = [
        HeldChecksTestCase {
            description: "no signal while reading: the list is kept",
            signals_during_read: 0,
            expected_held: Some("\"read\""),
        },
        HeldChecksTestCase {
            description: "a signal while reading: the list may predate it, so nothing is kept",
            signals_during_read: 1,
            expected_held: None,
        },
        HeldChecksTestCase {
            description: "several signals while reading: nothing is kept",
            signals_during_read: 3,
            expected_held: None,
        },
    ];
    for test_case in test_cases {
        let entry = TargetCache::default();
        let (generation, before) = held_checks(&entry);
        (0..test_case.signals_during_read).for_each(|_| drop_checks(&entry));
        keep_checks(
            &entry,
            generation,
            HeldCheckList {
                snapshot_id: None,
                json: CachedJson {
                    etag: Some("\"read\"".to_owned()),
                    body: Bytes::from_static(b"[]"),
                },
            },
        );
        let (_, held) = held_checks(&entry);
        assert_eq!(
            (
                before.is_none(),
                held.and_then(|list| list.json.etag).as_deref()
            ),
            (true, test_case.expected_held),
            "{}",
            test_case.description
        );
    }
}
