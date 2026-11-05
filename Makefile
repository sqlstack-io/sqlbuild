SHELL := /bin/bash

.PHONY: verify verify-quick verify-pg coverage cli-preview rust-check check-native-layering check-dupes-new check-native-stub check-build-features check-python-boundary check-release-gate check-docs-skill \
	check-e2e-shards test-e2e-duckdb test-e2e-duckdb-build-core \
	test-e2e-duckdb-build-incremental \
	test-e2e-duckdb-cli-data test-e2e-duckdb-cli test-e2e-duckdb-inspection \
	test-e2e-duckdb-integrations test-e2e-performance \
	test-e2e-cold-compile-performance test-e2e-cache-compile-performance \
	test-e2e-dense-compile-performance test-e2e-varied-cache-performance \
	compiler-differential compiler-differential-dense \
	compiler-baselines planner-differential

format:
	uv run ruff format .


SCENE ?= all
PREVIEW_ARGS ?=

cli-preview:
	uv run python -m scripts.preview_cli $(SCENE) $(PREVIEW_ARGS)


lint:
	uv run ruff check --fix .


type:
	uv run ty check src tests


.PHONY: dev-native
dev-native:
	uv run --no-sync maturin develop --profile dev --skip-install

.PHONY: fast-native
fast-native:
	uv run --no-sync maturin develop --profile fast --skip-install

COMPOSE_BUILD_PROFILE ?= release
COMPOSE_BUILD_NETWORK ?= default
COMPOSE_PYTHON_VERSION ?= 3.12

.PHONY: test-compose
test-compose:
	uv run --no-sync maturin build --profile $(COMPOSE_BUILD_PROFILE) --compatibility linux --out dist
	uv run --no-sync maturin build --manifest-path crates/sqlbuild-cli/Cargo.toml --profile $(COMPOSE_BUILD_PROFILE) --compatibility linux --out dist
	$(MAKE) test-compose-packaged

.PHONY: test-compose-packaged
test-compose-packaged:
	docker build --network $(COMPOSE_BUILD_NETWORK) --file deploy/Dockerfile --target server --build-arg PYTHON_VERSION=$(COMPOSE_PYTHON_VERSION) --tag sqlbuild-server:test .
	docker build --network $(COMPOSE_BUILD_NETWORK) --file deploy/Dockerfile --target location --build-arg PYTHON_VERSION=$(COMPOSE_PYTHON_VERSION) --build-arg SQLBUILD_GIT_REVISION=$$(git rev-parse HEAD) --tag sqlbuild-location:test .
	SQLBUILD_REQUIRE_COMPOSE_TESTS=1 uv run --no-sync pytest tests/integration/src/sqlbuild/compiler/compiled_project/publication/test_compose.py -n auto --dist loadfile -vv

rust-check: rust-lint
	uv run fensu check
	$(MAKE) test-native

NATIVE_TEST_ARGS ?= --workspace --all-features

.PHONY: test-native
test-native:
	uv run --no-sync python -m scripts.run_native_tests $(NATIVE_TEST_ARGS)

.PHONY: rust-lint
rust-lint:
	cargo fmt --all --check
	cargo clippy --workspace --all-targets --all-features -- -D warnings


test:
	@mkdir -p /tmp/opencode
	@log="/tmp/opencode/test-$$(date +%Y%m%d-%H%M%S).log"; \
	echo "Logging to $$log"; \
	env PYTHONUNBUFFERED=1 SQLBUILD_CONCURRENCY=$(SQLBUILD_CONCURRENCY) uv run pytest tests/unit tests/integration -m "not real_warehouse and not dbt" -vv --color=yes -n auto --dist loadfile --max-worker-restart=0 2>&1 | tee "$$log"; \
	status=$${PIPESTATUS[0]}; \
	echo "TEST_EXIT=$$status (log: $$log)" | tee -a "$$log"; \
	exit $$status


test-all:
	@mkdir -p /tmp/opencode
	@log="/tmp/opencode/test-all-$$(date +%Y%m%d-%H%M%S).log"; \
	echo "Logging to $$log"; \
	env PYTHONUNBUFFERED=1 SQLBUILD_CONCURRENCY=$(SQLBUILD_CONCURRENCY) uv run pytest tests -m "not real_warehouse and not dbt" -vv --color=yes -n auto --dist loadfile 2>&1 | tee "$$log"; \
	status=$${PIPESTATUS[0]}; \
	echo "TEST_ALL_EXIT=$$status (log: $$log)" | tee -a "$$log"; \
	exit $$status


E2E_DUCKDB_MARKERS := not real_warehouse and not dbt and not performance
E2E_DUCKDB_PYTEST_ARGS := -m "$(E2E_DUCKDB_MARKERS)" -vv --color=yes -n auto --dist loadfile

E2E_DUCKDB_BUILD_CORE_PATHS := \
	tests/e2e/src/sqlbuild/cli/commands/main/build/no_tests_no_audits \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_attached_audit_gates.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_audit_execution_errors.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_audit_factory_failures.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_audit_failures.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_build.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_compile_json_behavior.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_dynamic_column_contract.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_dag_json_behavior.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_enum_contract.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_execution_limits.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_explicit_references.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_expression_sources.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_lifecycle_commands.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_manifest_artifact_gating.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_materialization_type_change.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_model_config_build.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_no_tests_no_audits_flags.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_plan_command_surface.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_python_hooks.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_python_node_selection.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_python_source_deferral.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_defer_migration_origin.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_sql_hook_reads.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_python_node_identity.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_sql_hook_identity.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_query_change_tracking.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_query_fingerprint_changes.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_query_propagation.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_remove_column_semantics.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_reusable_model_schemas.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_runtime_artifact_preservation.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_schema_backfill_behavior.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_scoped_declarations.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_selector_surface.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_table_function_dependency.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_target_concurrency.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_template_expressions.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_terminal_failure.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_no_load_sources.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_duckdb_storage_key.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_source_end_audits.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_python_check_outcomes.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_duckdb_attach.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_unbuilt_dependencies.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_validation_failures.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_quoted_mixed_case_columns.py

E2E_DUCKDB_BUILD_INCREMENTAL_PATHS := \
	tests/e2e/src/sqlbuild/cli/commands/main/build/own_change_replay \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_append_cursor_build.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_capped_microbatch_build.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_concurrent_microbatch_build.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_cursor_runtime_failures.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_dropped_relation_rebuild.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_empty_cursor_inputs.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_loader_watermark_build.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_microbatch_aware_cursor_date_input.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_microbatch_direct_lifecycle.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_microbatch_failure_windows.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_microbatch_batch_progress.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_microbatch_replay_backfill_lifecycle.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_mixed_timestamp_grain_replay.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_model_backed_cursor_build.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_replay_on_change_full_build.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_seed_watermark_build.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_snapshot_build.py \
	tests/e2e/src/sqlbuild/cli/commands/main/build/test_snapshot_validity_columns.py


E2E_DUCKDB_CLI_DATA_PATHS := \
	tests/e2e/src/sqlbuild/cli/commands/main/load \
	tests/e2e/src/sqlbuild/cli/commands/main/providers \
	tests/e2e/src/sqlbuild/cli/commands/main/scenario/test_authored_check_sql.py \
	tests/e2e/src/sqlbuild/cli/commands/main/scenario/test_scenario.py \
	tests/e2e/src/sqlbuild/cli/commands/main/scenario/test_capture.py \
	tests/e2e/src/sqlbuild/cli/commands/main/scenario/test_namespace.py \
	tests/e2e/src/sqlbuild/cli/commands/main/scenario/test_promotion.py \
	tests/e2e/src/sqlbuild/cli/commands/main/scenario/test_concurrency.py \
	tests/e2e/src/sqlbuild/cli/commands/main/scenario/test_interrupts.py \
	tests/e2e/src/sqlbuild/cli/commands/main/scenario/test_empty_fixture.py \
	tests/e2e/src/sqlbuild/cli/commands/main/scenario/test_expected_columns.py \
	tests/e2e/src/sqlbuild/cli/commands/main/scenario/test_helper_ctes.py \
	tests/e2e/src/sqlbuild/cli/commands/main/scenario/test_fixture_columns.py \
	tests/e2e/src/sqlbuild/cli/commands/main/scenario/test_local_variant.py \
	tests/e2e/src/sqlbuild/cli/commands/main/scenario/test_local_lambda.py \
	tests/e2e/src/sqlbuild/cli/commands/main/scenario/test_local_escaped_quotes.py \
	tests/e2e/src/sqlbuild/cli/commands/main/scenario/test_snapshot_staleness.py \
	tests/e2e/src/sqlbuild/cli/commands/main/scenario/test_unmocked_inputs.py

E2E_DUCKDB_CLI_PATHS := \
	tests/e2e/scripts/cli_preview \
	tests/e2e/scripts/compile_performance_ratio \
	tests/e2e/scripts/compiler_differential \
	tests/e2e/scripts/planner_differential \
	tests/e2e/scripts/release_performance \
	tests/e2e/src/sqlbuild/cli/commands/main/adapters \
	tests/e2e/src/sqlbuild/cli/commands/main/audit \
	tests/e2e/src/sqlbuild/cli/commands/main/bigquery \
	tests/e2e/src/sqlbuild/cli/commands/main/check \
	tests/e2e/src/sqlbuild/cli/commands/main/clickhouse \
	tests/e2e/src/sqlbuild/cli/commands/main/compile \
	tests/e2e/src/sqlbuild/cli/commands/main/databricks \
	tests/e2e/src/sqlbuild/cli/commands/main/dbt \
	tests/e2e/src/sqlbuild/cli/commands/main/debug \
	tests/e2e/src/sqlbuild/cli/commands/main/format/test_description_layout.py \
	tests/e2e/src/sqlbuild/cli/commands/main/format/test_format_cache.py \
	tests/e2e/src/sqlbuild/cli/commands/main/format/test_format_example_projects.py \
	tests/e2e/src/sqlbuild/cli/commands/main/format/test_rule_fixes.py \
	tests/e2e/src/sqlbuild/cli/commands/main/format/test_token_preservation.py \
	tests/e2e/src/sqlbuild/cli/commands/main/freshness \
	tests/e2e/src/sqlbuild/cli/commands/main/init \
	tests/e2e/src/sqlbuild/cli/commands/main/rules \
	tests/e2e/src/sqlbuild/cli/commands/main/lineage \
	tests/e2e/src/sqlbuild/cli/commands/main/model_migrations \
	tests/e2e/src/sqlbuild/cli/commands/main/motherduck \
	tests/e2e/src/sqlbuild/cli/commands/main/playground \
	tests/e2e/src/sqlbuild/cli/commands/main/postgres \
	tests/e2e/src/sqlbuild/cli/commands/main/query \
	tests/e2e/src/sqlbuild/cli/commands/main/refactor \
	tests/e2e/src/sqlbuild/cli/commands/main/scope \
	tests/e2e/src/sqlbuild/cli/commands/main/seed \
	tests/e2e/src/sqlbuild/cli/commands/main/skills \
	tests/e2e/src/sqlbuild/cli/commands/main/snowflake \
	tests/e2e/src/sqlbuild/cli/commands/main/sqlite \
	tests/e2e/src/sqlbuild/cli/commands/main/sqlserver \
	tests/e2e/src/sqlbuild/cli/commands/main/test \
	tests/e2e/src/sqlbuild/cli/commands/shared

E2E_DUCKDB_INSPECTION_PATHS := \
	tests/e2e/src/sqlbuild/cli/commands/main/clone \
	tests/e2e/src/sqlbuild/cli/commands/main/diff \
	tests/e2e/src/sqlbuild/cli/commands/main/janitor \
	tests/e2e/src/sqlbuild/cli/commands/main/plan \
	tests/e2e/src/sqlbuild/cli/commands/main/review

E2E_DUCKDB_INTEGRATIONS_PATHS := tests/e2e/src/sqlbuild/integrations

E2E_DUCKDB_PATHS := \
	$(E2E_DUCKDB_BUILD_CORE_PATHS) \
	$(E2E_DUCKDB_BUILD_INCREMENTAL_PATHS) \
	$(E2E_DUCKDB_CLI_DATA_PATHS) \
	$(E2E_DUCKDB_CLI_PATHS) \
	$(E2E_DUCKDB_INSPECTION_PATHS) \
	$(E2E_DUCKDB_INTEGRATIONS_PATHS)

define run_e2e_duckdb
	@mkdir -p /tmp/opencode
	@log="/tmp/opencode/$(1)-$$(date +%Y%m%d-%H%M%S).log"; \
	echo "Logging to $$log"; \
	echo "Reproduce locally: make $(1)"; \
	echo 'Pytest scope: $(2) -m "$(E2E_DUCKDB_MARKERS)"'; \
	{ \
		status=0; \
		run_step() { \
			label="$$1"; shift; \
			echo; \
			echo "==> $$label"; \
			"$$@"; \
			rc=$$?; \
			if [ $$rc -ne 0 ]; then \
				echo "STEP_FAILED[$$rc]: $$label"; \
				if [ $$status -eq 0 ]; then status=$$rc; fi; \
			fi; \
		}; \
		run_step "$(1)" env PYTHONUNBUFFERED=1 SQLBUILD_CONCURRENCY=$(SQLBUILD_CONCURRENCY) uv run pytest $(2) $(E2E_DUCKDB_PYTEST_ARGS); \
		exit $$status; \
	} 2>&1 | tee "$$log"; \
	status=$${PIPESTATUS[0]}; \
	echo "TEST_E2E_DUCKDB_EXIT=$$status (log: $$log)" | tee -a "$$log"; \
	exit $$status
endef

test-e2e-duckdb:
	$(call run_e2e_duckdb,$@,$(E2E_DUCKDB_PATHS))

test-e2e-duckdb-build-core:
	$(call run_e2e_duckdb,$@,$(E2E_DUCKDB_BUILD_CORE_PATHS))

test-e2e-duckdb-build-incremental:
	$(call run_e2e_duckdb,$@,$(E2E_DUCKDB_BUILD_INCREMENTAL_PATHS))


test-e2e-duckdb-cli-data:
	$(call run_e2e_duckdb,$@,$(E2E_DUCKDB_CLI_DATA_PATHS))

test-e2e-duckdb-cli:
	$(call run_e2e_duckdb,$@,$(E2E_DUCKDB_CLI_PATHS))

test-e2e-duckdb-inspection:
	$(call run_e2e_duckdb,$@,$(E2E_DUCKDB_INSPECTION_PATHS))

test-e2e-duckdb-integrations:
	$(call run_e2e_duckdb,$@,$(E2E_DUCKDB_INTEGRATIONS_PATHS))

check-e2e-shards:
	bash scripts/check_e2e_shards.sh \
		--shard build-core $(E2E_DUCKDB_BUILD_CORE_PATHS) \
		--shard build-incremental $(E2E_DUCKDB_BUILD_INCREMENTAL_PATHS) \
		--shard cli-data $(E2E_DUCKDB_CLI_DATA_PATHS) \
		--shard cli $(E2E_DUCKDB_CLI_PATHS) \
		--shard inspection $(E2E_DUCKDB_INSPECTION_PATHS) \
		--shard integrations $(E2E_DUCKDB_INTEGRATIONS_PATHS)


test-e2e-performance:
	@mkdir -p /tmp/opencode
	@log="/tmp/opencode/test-e2e-performance-$$(date +%Y%m%d-%H%M%S).log"; \
	echo "Logging to $$log"; \
	env PYTHONUNBUFFERED=1 SQLBUILD_CONCURRENCY=$(SQLBUILD_CONCURRENCY) uv run pytest tests/e2e \
		-m "performance and not cold_compile_performance and not cache_compile_performance and not real_warehouse and not dbt" \
		-vv --log-cli-level=INFO --color=yes 2>&1 | tee "$$log"; \
	status=$${PIPESTATUS[0]}; \
	echo "TEST_E2E_PERFORMANCE_EXIT=$$status (log: $$log)" | tee -a "$$log"; \
	exit $$status


test-e2e-cold-compile-performance:
	test -n "$(SQLBUILD_BENCHMARK_MODELS)"
	env PYTHONUNBUFFERED=1 SQLBUILD_CONCURRENCY=4 uv run pytest \
		tests/e2e/src/sqlbuild/cli/commands/main/compile/test_fresh_process_compile_performance.py \
		tests/e2e/src/sqlbuild/cli/commands/main/compile/test_inspection_command_performance.py \
		tests/e2e/src/sqlbuild/cli/commands/main/compile/test_build_performance.py \
		-m cold_compile_performance -k "models_$(SQLBUILD_BENCHMARK_MODELS)" \
		-vv --log-cli-level=INFO --color=yes

.PHONY: test-e2e-diagnostic-compile-performance
test-e2e-diagnostic-compile-performance:
	env PYTHONUNBUFFERED=1 SQLBUILD_CONCURRENCY=4 uv run pytest \
		tests/e2e/src/sqlbuild/cli/commands/main/compile/test_diagnostic_compile_performance.py \
		-n auto --dist loadfile -m cold_compile_performance \
		-vv -rP --log-level=INFO --log-cli-level=INFO --color=yes

test-e2e-dense-compile-performance:
	test -n "$(SQLBUILD_BENCHMARK_MODELS)"
	env PYTHONUNBUFFERED=1 SQLBUILD_CONCURRENCY=4 uv run pytest \
		tests/e2e/src/sqlbuild/cli/commands/main/compile/test_dense_compile_performance.py \
		-n auto --dist loadfile \
		-m cold_compile_performance -k "models_$(SQLBUILD_BENCHMARK_MODELS)_" \
		-vv -rP --log-level=INFO --log-cli-level=INFO --color=yes
# DuckShop 10K runs after the dense guards, alone, so it never shares the memory limit with them.
	if [ "$(SQLBUILD_BENCHMARK_MODELS)" = "10000" ]; then \
		env PYTHONUNBUFFERED=1 uv run pytest benchmarks/duckshop-10k/tests -q --color=yes && \
		env PYTHONUNBUFFERED=1 SQLBUILD_CONCURRENCY=4 uv run pytest \
			tests/e2e/src/sqlbuild/cli/commands/main/compile/test_duckshop_10k_compile.py \
			-m cold_compile_performance -k "models_$(SQLBUILD_BENCHMARK_MODELS)_" \
			-vv -rP --log-level=INFO --log-cli-level=INFO --color=yes; \
	fi

test-e2e-cache-compile-performance:
	test -n "$(SQLBUILD_BENCHMARK_MODELS)"
	env PYTHONUNBUFFERED=1 SQLBUILD_CONCURRENCY=4 uv run pytest \
		tests/e2e/src/sqlbuild/cli/commands/main/compile/test_fresh_process_compile_cache_performance.py \
		-m cache_compile_performance -k "models_$(SQLBUILD_BENCHMARK_MODELS)" \
		-vv --log-cli-level=INFO --color=yes


test-e2e-varied-cache-performance:
	env PYTHONUNBUFFERED=1 SQLBUILD_CONCURRENCY=4 uv run pytest \
		tests/e2e/src/sqlbuild/cli/commands/main/compile/test_varied_compile_cache_performance.py \
		-n auto --dist loadfile -m cache_compile_performance \
		-vv -rP --log-level=INFO --log-cli-level=INFO --color=yes


COMPILER_DIFFERENTIAL_JOBS ?= 4
COMPILER_DIFFERENTIAL_SEEDS ?= 12
COMPILER_DIFFERENTIAL_DENSE_MODELS ?= 3000
COMPILER_DIFFERENTIAL_ARGS ?=
# Every output must match its golden. `make compiler-baselines` rewrites them
# (see scripts/compiler_differential/README.md).
COMPILER_DIFFERENTIAL_GATES := --goldens check

compiler-differential:
	env PYTHONUNBUFFERED=1 uv run python -m scripts.run_compiler_differential \
		--jobs $(COMPILER_DIFFERENTIAL_JOBS) --seeds $(COMPILER_DIFFERENTIAL_SEEDS) \
		$(COMPILER_DIFFERENTIAL_GATES) $(COMPILER_DIFFERENTIAL_ARGS)

# Rewrite the goldens from the CI run.
# Review the diff like code; see scripts/compiler_differential/README.md.
COMPILER_DIFFERENTIAL_CI_ARGS := --stage-captures --require-discovery-coverage --require-render-coverage
compiler-baselines:
	rm -rf tests/goldens/compiler
	$(MAKE) compiler-differential COMPILER_DIFFERENTIAL_ARGS="$(COMPILER_DIFFERENTIAL_CI_ARGS) \
		--require-analysis-coverage --goldens update"

compiler-differential-dense:
	env PYTHONUNBUFFERED=1 uv run python -m scripts.run_compiler_differential \
		--corpus dense --dense-models $(COMPILER_DIFFERENTIAL_DENSE_MODELS) \
		--jobs $(COMPILER_DIFFERENTIAL_JOBS) $(COMPILER_DIFFERENTIAL_ARGS)

# Stateful planner differential (see scripts/planner_differential/README.md): record a
# baseline on the tree before a planner change, then check the changed tree against it.
PLANNER_DIFFERENTIAL_MODE ?= check
PLANNER_DIFFERENTIAL_JOBS ?= 2
PLANNER_DIFFERENTIAL_SEEDS ?= 6
PLANNER_DIFFERENTIAL_ARGS ?=
planner-differential:
	env PYTHONUNBUFFERED=1 uv run python -m scripts.run_planner_differential \
		$(PLANNER_DIFFERENTIAL_MODE) --jobs $(PLANNER_DIFFERENTIAL_JOBS) \
		--seeds $(PLANNER_DIFFERENTIAL_SEEDS) $(PLANNER_DIFFERENTIAL_ARGS)


skills:
	uv run python -m scripts.generate_docs_skill
	uv run fensu skills

# Installs this checkout's development skill into your user-level OpenCode skills; never run by CI.
.PHONY: install-global-skill
install-global-skill:
	uv run sqb skills --global --target opencode


DBT_EXECUTABLE ?= dbt
export DBT_EXECUTABLE

SQLBUILD_CONCURRENCY ?= 8



test-dbt:
	@mkdir -p /tmp/opencode
	@log=/tmp/opencode/test-dbt-$$(date +%Y%m%d-%H%M%S).log; \
	echo "Logging to $$log (DBT_EXECUTABLE=$(DBT_EXECUTABLE))"; \
	uv run pytest tests/integration/src/sqlbuild/integrations/dbt tests/e2e/src/sqlbuild/cli/commands/main/dbt -m "dbt and not real_warehouse" -vv --color=yes -n auto --dist load 2>&1 | tee "$$log"; \
	status=$${PIPESTATUS[0]}; \
	echo "Full output saved to $$log"; \
	exit $$status


DBT_TEST_PATHS := tests/integration/src/sqlbuild/integrations/dbt tests/e2e/src/sqlbuild/cli/commands/main/dbt


test-dbt-real:
	@mkdir -p /tmp/opencode
	@log="/tmp/opencode/test-dbt-real-$$(date +%Y%m%d-%H%M%S).log"; \
	echo "Logging to $$log"; \
	env PYTHONUNBUFFERED=1 SQLBUILD_CONCURRENCY=$(SQLBUILD_CONCURRENCY) uv run pytest $(DBT_TEST_PATHS) -m "dbt and real_warehouse" -vv --color=yes -n auto --dist load 2>&1 | tee "$$log"; \
	status=$${PIPESTATUS[0]}; \
	echo "TEST_DBT_REAL_EXIT=$$status (log: $$log)" | tee -a "$$log"; \
	exit $$status


test-dbt-real-snowflake:
	uv run pytest $(DBT_TEST_PATHS) -m "dbt and real_warehouse and snowflake" -vv -n auto --dist load


test-dbt-real-bigquery:
	uv run pytest $(DBT_TEST_PATHS) -m "dbt and real_warehouse and bigquery" -vv -n auto --dist load


test-dbt-real-databricks:
	uv run pytest $(DBT_TEST_PATHS) -m "dbt and real_warehouse and databricks" -vv -n auto --dist load


test-dbt-real-postgres:
	TESTCONTAINERS_RYUK_DISABLED=true uv run pytest $(DBT_TEST_PATHS) -m "dbt and real_warehouse and postgres" -vv -n auto --dist load


test-dbt-real-sqlserver:
	uv run pytest $(DBT_TEST_PATHS) -m "dbt and real_warehouse and sqlserver" -vv -n auto --dist load


test-dbt-real-motherduck:
	uv run pytest $(DBT_TEST_PATHS) -m "dbt and real_warehouse and motherduck" -vv -n auto --dist load


waffle-shop:
	cd tests/e2e/fixtures/waffle_shop && $${SHELL:-/bin/sh}


test-real:
	@mkdir -p /tmp/opencode
	@log="/tmp/opencode/test-real-$$(date +%Y%m%d-%H%M%S).log"; \
	echo "Logging to $$log"; \
	env PYTHONUNBUFFERED=1 SQLBUILD_CONCURRENCY=$(SQLBUILD_CONCURRENCY) uv run pytest tests -m real_warehouse -vv --color=yes -n auto --dist load 2>&1 | tee "$$log"; \
	status=$${PIPESTATUS[0]}; \
	echo "TEST_REAL_EXIT=$$status (log: $$log)" | tee -a "$$log"; \
	exit $$status


test-real-all: test-real


test-real-snowflake:
	@mkdir -p /tmp/opencode
	@log="/tmp/opencode/test-real-snowflake-$$(date +%Y%m%d-%H%M%S).log"; \
	echo "Logging to $$log"; \
	env PYTHONUNBUFFERED=1 SQLBUILD_CONCURRENCY=$(SQLBUILD_CONCURRENCY) uv run pytest tests -m "real_warehouse and snowflake" -vv --color=yes -n auto --dist load 2>&1 | tee "$$log"; \
	status=$${PIPESTATUS[0]}; \
	echo "TEST_REAL_SNOWFLAKE_EXIT=$$status (log: $$log)" | tee -a "$$log"; \
	exit $$status


test-real-bigquery:
	@mkdir -p /tmp/opencode
	@log="/tmp/opencode/test-real-bigquery-$$(date +%Y%m%d-%H%M%S).log"; \
	echo "Logging to $$log"; \
	env PYTHONUNBUFFERED=1 SQLBUILD_CONCURRENCY=$(SQLBUILD_CONCURRENCY) uv run pytest tests -m "real_warehouse and bigquery" -vv --color=yes -n auto --dist load 2>&1 | tee "$$log"; \
	status=$${PIPESTATUS[0]}; \
	echo "TEST_REAL_BIGQUERY_EXIT=$$status (log: $$log)" | tee -a "$$log"; \
	exit $$status


test-real-databricks:
	@mkdir -p /tmp/opencode
	@log="/tmp/opencode/test-real-databricks-$$(date +%Y%m%d-%H%M%S).log"; \
	echo "Logging to $$log"; \
	env PYTHONUNBUFFERED=1 SQLBUILD_CONCURRENCY=$(SQLBUILD_CONCURRENCY) uv run pytest tests -m "real_warehouse and databricks" -vv --color=yes -n auto --dist load 2>&1 | tee "$$log"; \
	status=$${PIPESTATUS[0]}; \
	echo "TEST_REAL_DATABRICKS_EXIT=$$status (log: $$log)" | tee -a "$$log"; \
	exit $$status


test-real-postgres:
	@mkdir -p /tmp/opencode
	@log="/tmp/opencode/test-real-postgres-$$(date +%Y%m%d-%H%M%S).log"; \
	echo "Logging to $$log"; \
	env PYTHONUNBUFFERED=1 TESTCONTAINERS_RYUK_DISABLED=true SQLBUILD_CONCURRENCY=$(SQLBUILD_CONCURRENCY) uv run pytest tests -m "real_warehouse and postgres" -vv --color=yes -n auto --dist load 2>&1 | tee "$$log"; \
	status=$${PIPESTATUS[0]}; \
	echo "TEST_REAL_POSTGRES_EXIT=$$status (log: $$log)" | tee -a "$$log"; \
	exit $$status


test-real-clickhouse:
	@mkdir -p /tmp/opencode
	@log="/tmp/opencode/test-real-clickhouse-$$(date +%Y%m%d-%H%M%S).log"; \
	echo "Logging to $$log"; \
	env PYTHONUNBUFFERED=1 TESTCONTAINERS_RYUK_DISABLED=true SQLBUILD_CONCURRENCY=$(SQLBUILD_CONCURRENCY) uv run pytest tests -m "real_warehouse and clickhouse" -vv --color=yes -n auto --dist loadfile 2>&1 | tee "$$log"; \
	status=$${PIPESTATUS[0]}; \
	echo "TEST_REAL_CLICKHOUSE_EXIT=$$status (log: $$log)" | tee -a "$$log"; \
	exit $$status


test-real-sqlserver:
	@mkdir -p /tmp/opencode
	@log="/tmp/opencode/test-real-sqlserver-$$(date +%Y%m%d-%H%M%S).log"; \
	echo "Logging to $$log"; \
	env PYTHONUNBUFFERED=1 SQLBUILD_CONCURRENCY=$(SQLBUILD_CONCURRENCY) uv run pytest tests -m "real_warehouse and sqlserver" -vv --color=yes -n auto --dist load 2>&1 | tee "$$log"; \
	status=$${PIPESTATUS[0]}; \
	echo "TEST_REAL_SQLSERVER_EXIT=$$status (log: $$log)" | tee -a "$$log"; \
	exit $$status


test-real-motherduck:
	@mkdir -p /tmp/opencode
	@log="/tmp/opencode/test-real-motherduck-$$(date +%Y%m%d-%H%M%S).log"; \
	echo "Logging to $$log"; \
	env PYTHONUNBUFFERED=1 SQLBUILD_CONCURRENCY=$(SQLBUILD_CONCURRENCY) uv run pytest tests -m "real_warehouse and motherduck" -vv --color=yes -n auto --dist load 2>&1 | tee "$$log"; \
	status=$${PIPESTATUS[0]}; \
	echo "TEST_REAL_MOTHERDUCK_EXIT=$$status (log: $$log)" | tee -a "$$log"; \
	exit $$status


test-e2e-real-snowflake:
	@mkdir -p /tmp/opencode
	@log="/tmp/opencode/test-e2e-real-snowflake-$$(date +%Y%m%d-%H%M%S).log"; \
	echo "Logging to $$log"; \
	env PYTHONUNBUFFERED=1 SQLBUILD_CONCURRENCY=$(SQLBUILD_CONCURRENCY) uv run pytest tests/e2e -m "real_warehouse and snowflake" -vv --color=yes -n auto --dist load 2>&1 | tee "$$log"; \
	status=$${PIPESTATUS[0]}; \
	echo "TEST_E2E_REAL_SNOWFLAKE_EXIT=$$status (log: $$log)" | tee -a "$$log"; \
	exit $$status


test-e2e-real-bigquery:
	@mkdir -p /tmp/opencode
	@log="/tmp/opencode/test-e2e-real-bigquery-$$(date +%Y%m%d-%H%M%S).log"; \
	echo "Logging to $$log"; \
	env PYTHONUNBUFFERED=1 SQLBUILD_CONCURRENCY=$(SQLBUILD_CONCURRENCY) uv run pytest tests/e2e -m "real_warehouse and bigquery" -vv --color=yes -n auto --dist load 2>&1 | tee "$$log"; \
	status=$${PIPESTATUS[0]}; \
	echo "TEST_E2E_REAL_BIGQUERY_EXIT=$$status (log: $$log)" | tee -a "$$log"; \
	exit $$status


test-e2e-real-databricks:
	@mkdir -p /tmp/opencode
	@log="/tmp/opencode/test-e2e-real-databricks-$$(date +%Y%m%d-%H%M%S).log"; \
	echo "Logging to $$log"; \
	env PYTHONUNBUFFERED=1 SQLBUILD_CONCURRENCY=$(SQLBUILD_CONCURRENCY) uv run pytest tests/e2e -m "real_warehouse and databricks" -vv --color=yes -n auto --dist load 2>&1 | tee "$$log"; \
	status=$${PIPESTATUS[0]}; \
	echo "TEST_E2E_REAL_DATABRICKS_EXIT=$$status (log: $$log)" | tee -a "$$log"; \
	exit $$status


check-fensu:
	uv run fensu check


dupes:
	uv run fensu dupes


# Fails on duplicated code new since DUPES_BASE. A missing base fails too, so the check cannot
# silently disappear.
DUPES_BASE ?= origin/main

check-dupes-new:
	@if ! git rev-parse --verify --quiet "$(DUPES_BASE)^{commit}" >/dev/null; then \
		echo "error: new-duplicate check needs base $(DUPES_BASE); fetch it or set DUPES_BASE" >&2; \
		exit 2; \
	fi; \
	uv run fensu dupes --since "$(DUPES_BASE)" --fail-on-new


check:
	uv run ruff format .
	uv run ruff check --fix .
	uv run ty check src tests
	uv run pytest tests/unit/src/sqlbuild/adapter/contract/classes/strict_adapter/test_strict_adapter.py -q
	uv run fensu check


verify-pg:
	@mkdir -p /tmp/opencode
	@log="/tmp/opencode/verify-pg-$$(date +%Y%m%d-%H%M%S).log"; \
	{ \
		status=0; \
		run_verify_step() { \
			label="$$1"; \
			shift; \
			echo; \
			echo "==> $$label"; \
			"$$@"; \
			rc=$$?; \
			if [ $$rc -ne 0 ]; then \
				echo "STEP_FAILED[$$rc]: $$label"; \
				if [ $$status -eq 0 ]; then status=$$rc; fi; \
			fi; \
		}; \
		run_verify_step "ruff format" uv run ruff format .; \
		run_verify_step "ruff check" uv run ruff check --fix .; \
		run_verify_step "type check" uv run ty check src tests; \
		run_verify_step "tests" env PYTHONUNBUFFERED=1 TESTCONTAINERS_RYUK_DISABLED=true SQLBUILD_CONCURRENCY=$(SQLBUILD_CONCURRENCY) uv run pytest tests/unit tests/integration tests/e2e -m "((not real_warehouse and not dbt) or (dbt and not real_warehouse) or (real_warehouse and postgres)) and not performance" -vv --color=yes -n auto --dist loadfile; \
		run_verify_step "performance tests" env PYTHONUNBUFFERED=1 SQLBUILD_CONCURRENCY=$(SQLBUILD_CONCURRENCY) uv run pytest tests/e2e -m "performance and not real_warehouse and not dbt" -vv --color=yes; \
		run_verify_step "fensu" uv run fensu check; \
		exit $$status; \
	} 2>&1 | tee "$$log"; \
	status=$${PIPESTATUS[0]}; \
	echo "VERIFY_PG_EXIT=$$status (log: $$log)" | tee -a "$$log"; \
	exit $$status


coverage:
	@mkdir -p /tmp/opencode
	@log="/tmp/opencode/coverage-$$(date +%Y%m%d-%H%M%S).log"; \
	PYTHONUNBUFFERED=1 uv run pytest tests -m "not real_warehouse and not dbt" -vv --color=yes \
		--cov=src/sqlbuild --cov-branch \
		--cov-report=term-missing --cov-report=html --cov-report=xml \
		2>&1 | tee "$$log"; \
	status=$${PIPESTATUS[0]}; \
	echo "COVERAGE_EXIT=$$status (log: $$log)" | tee -a "$$log"; \
	exit $$status


verify:
	@mkdir -p /tmp/opencode
	@log="/tmp/opencode/verify-$$(date +%Y%m%d-%H%M%S).log"; \
	{ \
		status=0; \
		run_verify_step() { \
			label="$$1"; \
			shift; \
			echo; \
			echo "==> $$label"; \
			"$$@"; \
			rc=$$?; \
			if [ $$rc -ne 0 ]; then \
				echo "STEP_FAILED[$$rc]: $$label"; \
				if [ $$status -eq 0 ]; then status=$$rc; fi; \
			fi; \
		}; \
		run_verify_step "ruff format" uv run ruff format .; \
		run_verify_step "ruff check" uv run ruff check --fix .; \
		run_verify_step "type check" uv run ty check src tests; \
		run_verify_step "tests" env PYTHONUNBUFFERED=1 TESTCONTAINERS_RYUK_DISABLED=true SQLBUILD_CONCURRENCY=$(SQLBUILD_CONCURRENCY) uv run pytest tests/unit tests/integration tests/e2e -m "((not real_warehouse and not dbt) or (dbt and not real_warehouse)) and not performance" -vv --color=yes -n auto --dist loadfile; \
		run_verify_step "performance tests" env PYTHONUNBUFFERED=1 SQLBUILD_CONCURRENCY=$(SQLBUILD_CONCURRENCY) uv run pytest tests/e2e -m "performance and not real_warehouse and not dbt" -vv --color=yes; \
		run_verify_step "fensu" uv run fensu check; \
		exit $$status; \
	} 2>&1 | tee "$$log"; \
	status=$${PIPESTATUS[0]}; \
	echo "VERIFY_EXIT=$$status (log: $$log)" | tee -a "$$log"; \
	exit $$status


verify-quick:
	@mkdir -p /tmp/opencode
	@log="/tmp/opencode/verify-quick-$$(date +%Y%m%d-%H%M%S).log"; \
	{ \
		status=0; \
		run_verify_step() { \
			label="$$1"; \
			shift; \
			echo; \
			echo "==> $$label"; \
			"$$@"; \
			rc=$$?; \
			if [ $$rc -ne 0 ]; then \
				echo "STEP_FAILED[$$rc]: $$label"; \
				if [ $$status -eq 0 ]; then status=$$rc; fi; \
			fi; \
		}; \
		run_verify_step "ruff format" uv run ruff format .; \
		run_verify_step "ruff check" uv run ruff check --fix .; \
		run_verify_step "type check" uv run ty check src tests; \
		run_verify_step "tests" env PYTHONUNBUFFERED=1 TESTCONTAINERS_RYUK_DISABLED=true SQLBUILD_CONCURRENCY=$(SQLBUILD_CONCURRENCY) uv run pytest tests/unit tests/integration -m "((not real_warehouse and not dbt) or (dbt and not real_warehouse)) and not performance" -vv --color=yes -n auto --dist loadfile; \
		run_verify_step "fensu" uv run fensu check; \
		exit $$status; \
	} 2>&1 | tee "$$log"; \
	status=$${PIPESTATUS[0]}; \
	echo "VERIFY_QUICK_EXIT=$$status (log: $$log)" | tee -a "$$log"; \
	exit $$status


check-ci: check-dupes-new check-e2e-shards check-native-layering check-native-stub check-build-features check-python-boundary check-release-gate check-docs-skill rust-lint
	uv run ruff format --check .
	uv run ruff check .
	uv run ty check src tests
	uv run pytest tests/unit/src/sqlbuild/adapter/contract/classes/strict_adapter/test_strict_adapter.py -q
	uv run fensu check


check-native-layering:
	uv run python -m scripts.check_native_layering


check-native-stub:
	uv run python -m scripts.check_native_stub


check-build-features:
	uv run python -m scripts.check_build_features


check-python-boundary:
	uv run python -m scripts.check_python_boundary


check-release-gate:
	uv run python -m scripts.check_release_gate


check-docs-skill:
	uv run python -m scripts.check_docs_skill


check-pr-metadata:
	@test -n "$(PR_TITLE)" || { echo "PR_TITLE is required" >&2; exit 2; }
	@test -n "$(PR_BODY_FILE)" || { echo "PR_BODY_FILE is required" >&2; exit 2; }
	uv run python -m scripts.validate_pr_metadata \
		--title "$(PR_TITLE)" \
		--body-file "$(PR_BODY_FILE)"


verify-ci:
	uv run ruff format --check .
	uv run ruff check .
	uv run ty check src tests
	uv run pytest tests/unit tests/integration -m "not real_warehouse and not dbt" -vv -n auto --dist loadfile
	uv run pytest tests/e2e -m "not real_warehouse and not dbt and not performance" -vv -n auto --dist loadfile
	uv run pytest tests/e2e -m "performance and not real_warehouse and not dbt" -vv
	uv run fensu check
