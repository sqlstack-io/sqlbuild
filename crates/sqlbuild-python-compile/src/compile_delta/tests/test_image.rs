use sqlbuild_cache::project_reuse::types::ImageSections;

use crate::compile_delta::models::ImageGate;
use crate::compile_delta::models::ProjectImage;
use crate::compile_delta::tests::helpers::{
    current_format, decoded_rules, encoded, facts, image, sparse_image, with_other_format,
    with_truncated_facts, without_rule_digests,
};
use crate::compile_delta::tests::test_types::{DamagedImageTestCase, ImageRoundTripTestCase};

#[test]
fn given_project_image_when_encoded_and_decoded_then_every_fact_and_the_rules_survive() {
    let test_cases = [
        ImageRoundTripTestCase {
            description: "every fact and a retained rules request",
            image,
            expected_model_subjects: true,
            expected_rule_models: Some(0),
        },
        ImageRoundTripTestCase {
            description: "no optional facts and no rules request",
            image: sparse_image,
            expected_model_subjects: false,
            expected_rule_models: None,
        },
    ];
    for test_case in test_cases {
        let sections: ImageSections = encoded((test_case.image)());

        let gate: Option<ImageGate> = ImageGate::decode(&sections);
        let gate_facts: Option<(bool, bool, bool)> = gate
            .as_ref()
            .map(|gate| (gate.json_output, gate.has_errors, gate.has_model_subjects));
        let decoded: Option<ProjectImage> =
            gate.and_then(|gate| ProjectImage::decode(sections, gate));
        let decoded_facts: Option<(String, String)> = decoded
            .as_ref()
            .map(|image| (image.format.clone(), facts(image)));
        let rule_models: Option<usize> = decoded
            .and_then(decoded_rules)
            .map(|rules| rules.request.models.len() + rules.digests.len());

        assert_eq!(
            gate_facts,
            Some((true, false, test_case.expected_model_subjects)),
            "{}",
            test_case.description
        );
        assert_eq!(
            decoded_facts,
            Some((current_format(), facts(&(test_case.image)()))),
            "{}",
            test_case.description
        );
        assert_eq!(
            rule_models, test_case.expected_rule_models,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_damaged_image_when_decoded_then_it_is_never_read() {
    let test_cases = [
        DamagedImageTestCase {
            description: "intact",
            damage: |_| {},
            expected_gate: true,
            expected_image: true,
        },
        DamagedImageTestCase {
            description: "truncated facts",
            damage: with_truncated_facts,
            expected_gate: true,
            expected_image: false,
        },
        DamagedImageTestCase {
            description: "rules request without its digests",
            damage: without_rule_digests,
            expected_gate: true,
            expected_image: false,
        },
        DamagedImageTestCase {
            description: "gate of another format",
            damage: with_other_format,
            expected_gate: false,
            expected_image: false,
        },
    ];
    for test_case in test_cases {
        let mut sections: ImageSections = encoded(image());
        (test_case.damage)(&mut sections);

        let gate: Option<ImageGate> = ImageGate::decode(&sections);
        let gate_read: bool = gate.is_some();
        let decoded: Option<ProjectImage> =
            gate.and_then(|gate| ProjectImage::decode(sections, gate));

        assert_eq!(
            gate_read, test_case.expected_gate,
            "{}",
            test_case.description
        );
        assert_eq!(
            decoded.is_some(),
            test_case.expected_image,
            "{}",
            test_case.description
        );
    }
}
