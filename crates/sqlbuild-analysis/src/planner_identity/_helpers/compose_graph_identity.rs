//! A node identity composed from its local hash and the hashes of its available upstreams.

use sqlbuild_core::fingerprint_identity::main::stable_identity_hash::stable_identity_hash;

/// The hash of the local hash and each upstream hash, one per line.
pub(crate) fn compose_graph_identity(local_hash: &str, upstream_hashes: &[&str]) -> String {
    let mut text: String = local_hash.to_owned();
    for upstream_hash in upstream_hashes {
        text.push('\n');
        text.push_str(upstream_hash);
    }
    stable_identity_hash(&text)
}
