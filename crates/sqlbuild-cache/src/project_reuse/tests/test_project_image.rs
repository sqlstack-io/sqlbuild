use std::collections::{HashMap, HashSet};

use crate::project_reuse::_helpers::entry::{read_delta_base, read_delta_output, write_entry};
use crate::project_reuse::_helpers::files::{project_file_changes, unverified_paths};
use crate::project_reuse::models::StoredProjectFile;
use crate::project_reuse::tests::helpers::{
    image, image_file, stamp, stored_inputs, stored_output,
};
use crate::project_reuse::tests::test_types::{
    ChangedFileTestCase, ImagePruneTestCase, ImageReadTestCase, ImageRestoreTestCase,
};
use crate::project_reuse::types::ImageSections;

#[test]
fn given_stored_compile_with_image_when_reading_delta_base_then_only_an_intact_image_is_read() {
    let test_cases = [
        ImageReadTestCase {
            description: "intact image",
            edit: |_| {},
            expected_read: true,
        },
        ImageReadTestCase {
            description: "missing image",
            edit: |path| std::fs::remove_file(image_file(path)).expect("removed"),
            expected_read: false,
        },
        ImageReadTestCase {
            description: "damaged image",
            edit: |path| {
                let mut bytes = std::fs::read(image_file(path)).expect("image");
                let last = bytes.len() - 1;
                bytes[last] ^= 0xff;
                std::fs::write(image_file(path), bytes).expect("damaged");
            },
            expected_read: false,
        },
        ImageReadTestCase {
            description: "image of another compile",
            edit: |path| {
                let other = path.with_file_name("other.store");
                let mut sections = image();
                sections[0].1 = b"{\"models\": [\"customers\"]}".to_vec();
                write_entry(
                    &other,
                    (&stored_inputs(), &stored_output(), Some(&sections)),
                    8,
                )
                .expect("other compile");
                std::fs::copy(image_file(&other), image_file(path)).expect("replaced");
            },
            expected_read: false,
        },
    ];
    for test_case in test_cases {
        let folder = tempfile::tempdir().expect("store folder");
        let path = folder.path().join("reuse/slot.store");
        write_entry(
            &path,
            (&stored_inputs(), &stored_output(), Some(&image())),
            8,
        )
        .expect("stored compile");
        (test_case.edit)(&path);

        let read = read_delta_base(&path).and_then(|base| {
            let output = read_delta_output(&path, &base)?;
            Some((base.inputs, output, base.image))
        });

        assert_eq!(
            read,
            test_case
                .expected_read
                .then(|| (stored_inputs(), stored_output(), image())),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_stored_compile_with_image_when_storing_again_then_only_a_new_image_is_kept() {
    let test_cases = [
        ImageRestoreTestCase {
            description: "storing without an image removes the stale one",
            stored_again: None,
            expected_image_kept: false,
        },
        ImageRestoreTestCase {
            description: "storing with an image keeps it",
            stored_again: Some(image),
            expected_image_kept: true,
        },
    ];
    for test_case in test_cases {
        let folder = tempfile::tempdir().expect("store folder");
        let path = folder.path().join("slot.store");
        write_entry(
            &path,
            (&stored_inputs(), &stored_output(), Some(&image())),
            8,
        )
        .expect("stored");
        let again: Option<ImageSections> = test_case.stored_again.map(|image| image());

        write_entry(
            &path,
            (&stored_inputs(), &stored_output(), again.as_ref()),
            8,
        )
        .expect("stored again");

        assert_eq!(
            image_file(&path).exists(),
            test_case.expected_image_kept,
            "{}",
            test_case.description
        );
        assert_eq!(
            read_delta_base(&path).is_some(),
            test_case.expected_image_kept,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_images_of_many_slots_when_storing_then_only_the_most_recently_used_two_are_kept() {
    let test_cases = [
        ImagePruneTestCase {
            description: "the two newest images are kept",
            slots: 4,
            read: &[],
            expected_images: &["slot3.image", "slot4.image"],
        },
        ImagePruneTestCase {
            description: "a read image counts as used",
            slots: 3,
            read: &[1],
            expected_images: &["slot1.image", "slot3.image"],
        },
    ];
    for test_case in test_cases {
        let folder = tempfile::tempdir().expect("store folder");
        for slot in 0..test_case.slots {
            let path = folder.path().join(format!("slot{slot}.store"));
            write_entry(
                &path,
                (&stored_inputs(), &stored_output(), Some(&image())),
                8,
            )
            .expect("stored");
            let aged = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(slot + 1);
            std::fs::File::options()
                .write(true)
                .open(image_file(&path))
                .and_then(|file| file.set_modified(aged))
                .expect("aged image");
        }
        for slot in test_case.read {
            assert!(read_delta_base(&folder.path().join(format!("slot{slot}.store"))).is_some());
        }
        let newest = folder.path().join(format!("slot{}.store", test_case.slots));
        write_entry(
            &newest,
            (&stored_inputs(), &stored_output(), Some(&image())),
            8,
        )
        .expect("stored again");

        let mut kept: Vec<String> = std::fs::read_dir(folder.path())
            .expect("listed")
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(".image"))
            .collect();
        kept.sort();

        assert_eq!(kept, test_case.expected_images, "{}", test_case.description);
    }
}

#[test]
fn given_project_file_when_computing_delta_changes_then_only_content_changes_count() {
    let test_cases = [
        ChangedFileTestCase {
            description: "settled equal stamp is unchanged without reading",
            stored_digest: None,
            racy: false,
            mtime_shift_ns: 0,
            current_digest: None,
            expected_changed: &[],
        },
        ChangedFileTestCase {
            description: "moved stamp with equal bytes is unchanged",
            stored_digest: Some("b3:one"),
            racy: false,
            mtime_shift_ns: 1,
            current_digest: Some("b3:one"),
            expected_changed: &[],
        },
        ChangedFileTestCase {
            description: "moved stamp with new bytes is changed",
            stored_digest: Some("b3:one"),
            racy: false,
            mtime_shift_ns: 1,
            current_digest: Some("b3:two"),
            expected_changed: &["sql/models/orders.sql"],
        },
        ChangedFileTestCase {
            description: "racy equal stamp with new bytes is changed",
            stored_digest: Some("b3:one"),
            racy: true,
            mtime_shift_ns: 0,
            current_digest: Some("b3:two"),
            expected_changed: &["sql/models/orders.sql"],
        },
        ChangedFileTestCase {
            description: "moved stamp without a stored digest is changed",
            stored_digest: None,
            racy: false,
            mtime_shift_ns: 1,
            current_digest: Some("b3:one"),
            expected_changed: &["sql/models/orders.sql"],
        },
        ChangedFileTestCase {
            description: "unreadable moved file is changed",
            stored_digest: Some("b3:one"),
            racy: false,
            mtime_shift_ns: 1,
            current_digest: None,
            expected_changed: &["sql/models/orders.sql"],
        },
    ];
    for test_case in test_cases {
        let stored = vec![StoredProjectFile {
            stamp: stamp("sql/models/orders.sql", "f", None),
            digest: test_case.stored_digest.map(str::to_owned),
            racy: test_case.racy,
        }];
        let mut current = stamp("sql/models/orders.sql", "f", None);
        current.mtime_ns += test_case.mtime_shift_ns;
        let current = vec![current, stamp("sql/models/customers.sql", "f", None)];
        let digests: HashMap<String, String> = test_case
            .current_digest
            .map(|digest| HashMap::from([("sql/models/orders.sql".to_owned(), digest.to_owned())]))
            .unwrap_or_default();
        let unverified: HashSet<String> = unverified_paths(&stored, &current);

        let changes = project_file_changes(&stored, &current, (&digests, &unverified));

        assert_eq!(
            changes.changed, test_case.expected_changed,
            "{}",
            test_case.description
        );
        assert_eq!(
            (changes.added, changes.removed),
            (
                vec!["sql/models/customers.sql".to_owned()],
                Vec::<String>::new()
            ),
            "{}",
            test_case.description
        );
    }
}
