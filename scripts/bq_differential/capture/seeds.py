"""Run the compiler differential's seed corpus with every seed generated for BigQuery.

Usage (from the repository root): python scripts/bq_differential/capture/seeds.py --corpus seeds
--seeds 30 --stage-captures --work-dir <dir> --jobs 4. The connection and target values are
placeholders; nothing connects to BigQuery.
"""

import sys

from scripts.compiler_differential._helpers.corpus import corpus
from scripts.compiler_differential.classes import project_builder

project_builder.GENERATOR_DIALECT_CONNECTIONS["bigquery"] = (
    'project = "example-project"',
    'location = "US"',
)
project_builder.GENERATOR_DIALECT_TARGETS["bigquery"] = ('database = "example-project"',)
corpus.ANALYSIS_DIALECT_VARIANTS = ()
corpus._seed_project = lambda seed: corpus._dialect_seed_project(seed=seed, dialect="bigquery")

from scripts.run_compiler_differential import main  # noqa: E402

raise SystemExit(main(sys.argv[1:]))
