//! Deferred-clone ancestors in depth-first order, with caller-owned keys and predicates.

use crate::graph_walk::models::ClonePolicy;
use crate::graph_walk::types::NodeSet;

/// Find clone boundaries, or the skipped views rebuilt over them, without crossing a host boundary.
pub fn clone_ancestors<K, S, N, P, C, V>(
    starts: impl IntoIterator<Item = Result<K, S::Error>>,
    (visited, found): (&mut S, &mut S),
    mut parents: N,
    policy: ClonePolicy<P, C, V>,
) -> Result<(), S::Error>
where
    K: Clone,
    S: NodeSet<K>,
    N: FnMut(&K) -> Result<Vec<K>, S::Error>,
    P: FnMut(&K) -> Result<bool, S::Error>,
    C: FnMut(&K) -> Result<bool, S::Error>,
    V: FnMut(&K) -> Result<bool, S::Error>,
{
    let ClonePolicy {
        mut selected,
        mut clonable,
        mut view,
        skipped_views,
    } = policy;
    for start in starts {
        let start = start?;
        if !visited.insert_new(&start)? {
            continue;
        }
        let mut stack = vec![(parents(&start)?, 0)];
        while let Some((neighbours, index)) = stack.last_mut() {
            let Some(parent) = neighbours.get(*index).cloned() else {
                stack.pop();
                continue;
            };
            *index += 1;
            let descend = if selected(&parent)? {
                true
            } else if skipped_views {
                let is_view = view(&parent)?;
                if is_view && clonable(&parent)? {
                    found.insert_new(&parent)?;
                }
                is_view
            } else if view(&parent)? {
                true
            } else {
                if clonable(&parent)? {
                    found.insert_new(&parent)?;
                }
                false
            };
            if descend && visited.insert_new(&parent)? {
                stack.push((parents(&parent)?, 0));
            }
        }
    }
    Ok(())
}
