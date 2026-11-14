use crate::models::Fault;
use crate::python_rules::models::PythonCacheEntry;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Cached findings by path, valid for one ruleset fingerprint.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct RuleCacheBucket<E = RuleCacheEntry> {
    pub fingerprint: String,
    pub entries: BTreeMap<String, E>,
}

impl<E> Default for RuleCacheBucket<E> {
    fn default() -> Self {
        Self {
            fingerprint: String::new(),
            entries: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct RuleCacheEntry {
    pub identity: String,
    pub faults: Vec<Fault>,
}

const FIXTURE_SUMMARIES_FILE: &str = "sql-test-fixtures.json";
const PYTHON_RULES_FILE: &str = "python.json";

/// Where the SQL-test fixture summaries sit, beside the bulk native rules cache.
pub(crate) fn fixture_summaries_path(project_dir: &Path, rules_cache_path: &str) -> PathBuf {
    let native: PathBuf = if rules_cache_path.is_empty() {
        project_dir.join("target/rules-cache/bulk/native.json")
    } else {
        PathBuf::from(rules_cache_path)
    };
    native.with_file_name(FIXTURE_SUMMARIES_FILE)
}

pub(crate) struct Cache {
    native_rules_path: PathBuf,
}

impl Cache {
    pub(crate) fn open(project_dir: &Path, rules_cache_path: &str) -> Result<Self, String> {
        let native_rules_path = if rules_cache_path.is_empty() {
            project_dir.join("target/rules-cache/bulk/native.json")
        } else {
            PathBuf::from(rules_cache_path)
        };
        Ok(Self { native_rules_path })
    }

    pub(crate) fn native_rules_bucket(
        &self,
        ruleset_identity: &str,
    ) -> Result<RuleCacheBucket, String> {
        read_bucket(&self.native_rules_path, ruleset_identity)
    }

    pub(crate) fn put_native_rules_bucket(
        &self,
        ruleset_identity: &str,
        bucket: &RuleCacheBucket,
    ) -> Result<(), String> {
        write_bucket(&self.native_rules_path, ruleset_identity, bucket)
    }

    /// The write `put_native_rules_bucket` would make, held until it is published.
    pub(crate) fn pending_native_rules_bucket(
        &self,
        ruleset_identity: &str,
        bucket: RuleCacheBucket,
    ) -> Result<PendingBucketWrite, String> {
        PendingBucketWrite::new(&self.native_rules_path, ruleset_identity, &bucket)
    }

    /// The Python rules' findings by file, kept beside the bulk native rules cache.
    pub(crate) fn python_rules_bucket(
        &self,
        ruleset_identity: &str,
    ) -> Result<RuleCacheBucket<PythonCacheEntry>, String> {
        read_bucket(&self.python_rules_path(), ruleset_identity)
    }

    /// The write of the Python rules' findings, held until it is published.
    pub(crate) fn pending_python_rules_bucket(
        &self,
        ruleset_identity: &str,
        bucket: &RuleCacheBucket<PythonCacheEntry>,
    ) -> Result<PendingBucketWrite, String> {
        PendingBucketWrite::new(&self.python_rules_path(), ruleset_identity, bucket)
    }

    fn python_rules_path(&self) -> PathBuf {
        self.native_rules_path.with_file_name(PYTHON_RULES_FILE)
    }
}

/// Rules cache writes that have not been made yet, made in order when published.
#[derive(Debug)]
pub(crate) struct PendingBucketWrite {
    writes: Vec<(PathBuf, Vec<u8>)>,
}

impl PendingBucketWrite {
    fn new<E: Clone + Serialize>(
        path: &Path,
        fingerprint: &str,
        bucket: &RuleCacheBucket<E>,
    ) -> Result<Self, String> {
        Ok(Self {
            writes: vec![(path.to_path_buf(), encoded_bucket(fingerprint, bucket)?)],
        })
    }

    /// These writes followed by `later`'s.
    pub(crate) fn then(mut self, later: Self) -> Self {
        self.writes.extend(later.writes);
        self
    }

    /// Make the writes exactly as the immediate `put` methods make them.
    pub(crate) fn publish(&self) -> Result<(), String> {
        self.writes
            .iter()
            .try_for_each(|(path, bytes)| write_bytes(path, bytes))
    }
}

fn read_bucket<E: DeserializeOwned>(
    path: &Path,
    fingerprint: &str,
) -> Result<RuleCacheBucket<E>, String> {
    let bytes = match std::fs::read(path) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(RuleCacheBucket {
                fingerprint: fingerprint.to_owned(),
                entries: BTreeMap::new(),
            });
        }
        Err(error) => return Err(format!("could not read bulk rules cache: {error}")),
    };
    let bucket: RuleCacheBucket<E> = match serde_json::from_slice(&bytes) {
        Ok(value) => value,
        Err(error) if error.is_syntax() || error.is_eof() || error.is_data() => {
            RuleCacheBucket::default()
        }
        Err(error) => return Err(format!("could not parse bulk rules cache: {error}")),
    };
    if bucket.fingerprint == fingerprint {
        Ok(bucket)
    } else {
        Ok(RuleCacheBucket {
            fingerprint: fingerprint.to_owned(),
            entries: BTreeMap::new(),
        })
    }
}

fn write_bucket(path: &Path, fingerprint: &str, bucket: &RuleCacheBucket) -> Result<(), String> {
    write_bytes(path, &encoded_bucket(fingerprint, bucket)?)
}

fn encoded_bucket<E: Clone + Serialize>(
    fingerprint: &str,
    bucket: &RuleCacheBucket<E>,
) -> Result<Vec<u8>, String> {
    let mut output = bucket.clone();
    output.fingerprint = fingerprint.to_owned();
    serde_json::to_vec(&output)
        .map_err(|error| format!("could not encode bulk rules cache: {error}"))
}

fn write_bytes(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "bulk rules cache path has no parent".to_owned())?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("could not create bulk rules cache: {error}"))?;
    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
    std::fs::write(&temporary, bytes)
        .map_err(|error| format!("could not write bulk rules cache: {error}"))?;
    std::fs::rename(&temporary, path)
        .map_err(|error| format!("could not publish bulk rules cache: {error}"))
}
