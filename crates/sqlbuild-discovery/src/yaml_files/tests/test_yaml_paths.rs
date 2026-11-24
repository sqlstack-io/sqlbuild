use crate::yaml_files::main::schema_yaml_paths::schema_yaml_paths;
use crate::yaml_files::main::source_yaml_paths::source_yaml_paths;
use crate::yaml_files::models::YamlPathFailure;
use crate::yaml_files::tests::helpers::{listed_paths, owned_paths};
use crate::yaml_files::tests::test_types::YamlPathsTestCase;

#[test]
fn given_sources_directory_when_listing_source_yaml_then_direct_yaml_children_are_sorted() {
    let test_cases = [
        YamlPathsTestCase {
            description: "yml and yaml files directly below sql/sources/ in path order",
            files: &[
                "sql/sources/b.yaml",
                "sql/sources/a.yml",
                "sql/sources/notes.txt",
                "sql/sources/.yml",
                "sql/sources/nested/c.yml",
            ],
            expected_paths: Ok(owned_paths(&["sql/sources/a.yml", "sql/sources/b.yaml"])),
        },
        YamlPathsTestCase {
            description: "no sources directory",
            files: &["sql/models/orders.sql"],
            expected_paths: Ok(Vec::new()),
        },
    ];
    for test_case in test_cases {
        let listed = listed_paths(test_case.files, |tree| {
            source_yaml_paths(tree).map_err(YamlPathFailure::Stage)
        });
        assert_eq!(
            listed, test_case.expected_paths,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_models_and_seeds_when_listing_schema_yaml_then_unscoped_schemas_and_seed_files_are_kept() {
    let test_cases = [
        YamlPathsTestCase {
            description: "unscoped schema files then seed declarations, each in path order",
            files: &[
                "sql/models/marts/schema.yml",
                "sql/models/schema.yml",
                "sql/models/marts/_sqlbuild/schemas/schema.yml",
                "sql/models/other.yml",
                "sql/seeds/products.yml",
                "sql/seeds/customers/customers.yml",
            ],
            expected_paths: Ok(owned_paths(&[
                "sql/models/marts/schema.yml",
                "sql/models/schema.yml",
                "sql/seeds/customers/customers.yml",
                "sql/seeds/products.yml",
            ])),
        },
        YamlPathsTestCase {
            description: "a seed declaration using .yaml fails naming the first one",
            files: &["sql/seeds/b.yaml", "sql/seeds/a.yaml", "sql/seeds/c.yml"],
            expected_paths: Err(YamlPathFailure::YamlSeedDeclaration(
                "sql/seeds/a.yaml".to_owned(),
            )),
        },
    ];
    for test_case in test_cases {
        let listed = listed_paths(test_case.files, schema_yaml_paths);
        assert_eq!(
            listed, test_case.expected_paths,
            "{}",
            test_case.description
        );
    }
}
