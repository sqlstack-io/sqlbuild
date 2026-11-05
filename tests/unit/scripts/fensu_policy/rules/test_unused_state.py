import itertools
from textwrap import dedent

import pytest

from scripts.fensu_policy._helpers.unused_state import (
    code_view,
    collect_reads,
    enum_variants,
    named_variants,
    schema_definitions,
    sql_read_fragments,
    string_literals,
    unread_definitions,
)
from scripts.fensu_policy.models import EnumVariant, StateDefinition
from tests.unit.scripts.fensu_policy.rules._test_types import (
    EnumVariantTestCase,
    UnreadStateTestCase,
)

ORDERS_SCHEMA: str = dedent(
    """
    -- Orders and their fulfillment attempts.
    CREATE TABLE orders (
        order_id BIGINT NOT NULL,
        status TEXT NOT NULL CHECK (status IN ('open', 'shipped')),
        retries BIGINT NOT NULL DEFAULT 0,
        note_json TEXT NOT NULL,
        PRIMARY KEY (order_id)
    );
    CREATE INDEX orders_open ON orders (status) WHERE retries > 0;
    ALTER TABLE orders ADD COLUMN shipped_at TIMESTAMPTZ;
    CREATE TABLE order_events (event_id BIGINT NOT NULL, order_id BIGINT NOT NULL);
    """
)


@pytest.mark.parametrize(
    "test_case",
    [
        UnreadStateTestCase(
            description="written columns, indexes, DELETE targets and comments are not reads",
            schema=ORDERS_SCHEMA,
            rust=dedent(
                """
                fn load() { query("SELECT order_id, status FROM orders WHERE shipped_at IS NULL"); }
                fn save() { query("UPDATE orders SET retries = 0, note_json = $1 WHERE order_id = $2"); }
                fn prune() { query("DELETE FROM order_events WHERE event_id < $1"); }
                // query("SELECT retries FROM orders")
                """
            ),
            expected_unread=(
                ("orders", "retries"),
                ("orders", "note_json"),
                ("order_events", None),
            ),
        ),
        UnreadStateTestCase(
            description="reads inside a test module do not count",
            schema=ORDERS_SCHEMA,
            rust=dedent(
                """
                fn load() { query("SELECT * FROM orders"); }
                #[cfg(test)]
                mod tests {
                    fn check() { query("SELECT event_id, order_id FROM order_events"); }
                }
                """
            ),
            expected_unread=(("order_events", None),),
        ),
        UnreadStateTestCase(
            description="an assignment's right side reads but an upsert's excluded value does not",
            schema=ORDERS_SCHEMA,
            rust=dedent(
                """
                fn bump() {
                    query("UPDATE orders SET retries = retries + 1 WHERE order_id = $1 RETURNING status");
                    query("INSERT INTO orders (order_id, note_json) VALUES ($1, $2) \\
                           ON CONFLICT (order_id) DO UPDATE SET note_json = excluded.note_json");
                    query("SELECT o.order_id FROM orders o JOIN order_events e ON e.order_id = o.order_id");
                }
                """
            ),
            expected_unread=(
                ("orders", "note_json"),
                ("orders", "shipped_at"),
                ("order_events", "event_id"),
            ),
        ),
        UnreadStateTestCase(
            description="a predicate assembled from fragments reads its columns",
            schema=ORDERS_SCHEMA,
            rust=dedent(
                """
                fn filter(shipped: bool) {
                    let predicate = if shipped { "shipped_at IS NOT NULL" } else { "retries = $1" };
                    query(&format!("SELECT order_id, status, note_json FROM orders WHERE {predicate}"));
                    query("SELECT event_id, order_id FROM order_events");
                    let key = "status";
                }
                """
            ),
            expected_unread=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_control_store_sql_when_collecting_reads_then_reports_unread_state(
    test_case: UnreadStateTestCase,
) -> None:
    definitions: list[StateDefinition] = schema_definitions(
        path="store/schema.sql", sql=test_case.schema
    )
    fragments: list[str] = [
        *sql_read_fragments(sql=test_case.schema),
        *string_literals(source=test_case.rust),
    ]
    unread: list[StateDefinition] = unread_definitions(
        definitions=definitions,
        reads=collect_reads(fragments=fragments, definitions=definitions),
    )

    assert tuple((definition.table, definition.column) for definition in unread) == (
        test_case.expected_unread
    )


@pytest.mark.parametrize(
    "test_case",
    [
        EnumVariantTestCase(
            description="paths, Self in an impl and glob imports name variants; text does not",
            files=(
                (
                    "crates/shop/src/models.rs",
                    dedent(
                        """
                        /// Mentions Outcome::Cancelled only in a comment.
                        #[derive(Deserialize)]
                        #[serde(rename_all = "snake_case")]
                        pub enum Outcome {
                            /// Waiting for stock.
                            Pending,
                            #[serde(rename = "a,b")]
                            Shipped { carrier: String, parcels: (u8, u8) },
                            Returned(Vec<(u8, u8)>),
                            Cancelled = 4,
                        }

                        impl Outcome {
                            fn label(self) -> &'static str {
                                match self { Self::Shipped { .. } => "shipped", _ => "Outcome::Cancelled" }
                            }
                        }

                        fn start() -> Outcome { Outcome::Pending }
                        """
                    ),
                ),
                (
                    "crates/shop/src/returns.rs",
                    dedent(
                        """
                        use crate::models::Outcome::*;

                        fn returned(outcome: &Outcome) -> bool { matches!(outcome, Returned(_)) }
                        """
                    ),
                ),
            ),
            expected_unnamed=(("Outcome", "Cancelled"),),
        ),
        EnumVariantTestCase(
            description="test-only use, macro-declared enums and #[from] variants",
            files=(
                (
                    "crates/shop/src/errors.rs",
                    dedent(
                        """
                        pub enum StoreError {
                            Io(#[from] std::io::Error),
                            Closed,
                            Locked,
                        }

                        fn fail() -> StoreError { StoreError::Locked }

                        ids! {
                            pub enum Generated { Never => "never" }
                        }

                        #[cfg(test)]
                        mod tests {
                            fn closed() -> super::StoreError { super::StoreError::Closed }
                        }
                        """
                    ),
                ),
            ),
            expected_unnamed=(("StoreError", "Closed"),),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_rust_enums_when_finding_named_variants_then_reports_unnamed_variants(
    test_case: EnumVariantTestCase,
) -> None:
    codes: dict[str, str] = {path: code_view(source=source) for path, source in test_case.files}
    variants: list[EnumVariant] = list(
        itertools.chain.from_iterable(
            enum_variants(path=path, code=code) for path, code in codes.items()
        )
    )
    unnamed: set[EnumVariant] = set(variants) - named_variants(variants=variants, codes=codes)

    assert {(variant.enum, variant.variant) for variant in unnamed} == set(
        test_case.expected_unnamed
    )
