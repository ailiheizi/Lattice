use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DagNode {
    pub msg_hash: Vec<u8>,
    pub prev_hashes: Vec<Vec<u8>>,
    pub received_ts: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderedNode {
    pub msg_hash: Vec<u8>,
    pub prev_hashes: Vec<Vec<u8>>,
    pub depth: u64,
    pub received_ts: u64,
}

pub fn is_outlier(node: &DagNode, known_hashes: &BTreeSet<Vec<u8>>) -> bool {
    node.prev_hashes.iter().any(|parent| !known_hashes.contains(parent))
}

pub fn missing_parents(node: &DagNode, known_hashes: &BTreeSet<Vec<u8>>) -> Vec<Vec<u8>> {
    let mut missing = BTreeSet::new();
    for parent in &node.prev_hashes {
        if !known_hashes.contains(parent) {
            missing.insert(parent.clone());
        }
    }
    missing.into_iter().collect()
}

pub fn missing_parents_for_nodes(
    nodes: &[DagNode],
    known_hashes: &BTreeSet<Vec<u8>>,
) -> Vec<Vec<u8>> {
    let mut missing = BTreeSet::new();
    for node in nodes {
        for parent in missing_parents(node, known_hashes) {
            missing.insert(parent);
        }
    }
    missing.into_iter().collect()
}

pub fn forward_extremities(nodes: &[DagNode]) -> Vec<Vec<u8>> {
    let mut candidates = BTreeSet::new();
    let mut referenced = BTreeSet::new();

    for node in nodes {
        candidates.insert(node.msg_hash.clone());
        for parent in &node.prev_hashes {
            referenced.insert(parent.clone());
        }
    }

    candidates
        .into_iter()
        .filter(|hash| !referenced.contains(hash))
        .collect()
}

pub fn deterministic_order(nodes: &[DagNode]) -> Vec<OrderedNode> {
    let index = index_nodes(nodes);
    let mut ordered = Vec::with_capacity(index.len());
    let mut available = BTreeSet::new();
    let mut in_degree = BTreeMap::new();
    let mut children = BTreeMap::<Vec<u8>, Vec<Vec<u8>>>::new();
    let mut depths = BTreeMap::<Vec<u8>, u64>::new();

    for (hash, node) in &index {
        let known_parents: Vec<Vec<u8>> = node
            .prev_hashes
            .iter()
            .filter(|parent| index.contains_key(*parent))
            .cloned()
            .collect();
        in_degree.insert(hash.clone(), known_parents.len() as u64);
        if known_parents.is_empty() {
            depths.insert(hash.clone(), 0);
            available.insert(sort_key(0, node.received_ts, hash));
        }
        for parent in known_parents {
            children.entry(parent).or_default().push(hash.clone());
        }
    }

    for hashes in children.values_mut() {
        hashes.sort();
    }

    while let Some(key) = available.pop_first() {
        let hash = key.msg_hash;
        let node = index.get(&hash).expect("node indexed");
        let depth = *depths.get(&hash).unwrap_or(&0);
        ordered.push(OrderedNode {
            msg_hash: hash.clone(),
            prev_hashes: node.prev_hashes.clone(),
            depth,
            received_ts: node.received_ts,
        });

        if let Some(next_children) = children.get(&hash) {
            for child_hash in next_children {
                let child = index.get(child_hash).expect("child indexed");
                let entry = depths.entry(child_hash.clone()).or_insert(0);
                *entry = (*entry).max(depth + 1);

                if let Some(remaining) = in_degree.get_mut(child_hash) {
                    *remaining -= 1;
                    if *remaining == 0 {
                        available.insert(sort_key(*entry, child.received_ts, child_hash));
                    }
                }
            }
        }
    }

    ordered
}

fn index_nodes(nodes: &[DagNode]) -> BTreeMap<Vec<u8>, DagNode> {
    let mut index = BTreeMap::new();
    for node in nodes {
        index.insert(node.msg_hash.clone(), node.clone());
    }
    index
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct SortKey {
    depth: u64,
    received_ts: u64,
    msg_hash: Vec<u8>,
}

fn sort_key(depth: u64, received_ts: u64, msg_hash: &[u8]) -> SortKey {
    SortKey {
        depth,
        received_ts,
        msg_hash: msg_hash.to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(hash: &[u8], parents: &[&[u8]], received_ts: u64) -> DagNode {
        DagNode {
            msg_hash: hash.to_vec(),
            prev_hashes: parents.iter().map(|parent| parent.to_vec()).collect(),
            received_ts,
        }
    }

    fn hashes(nodes: &[OrderedNode]) -> Vec<Vec<u8>> {
        nodes.iter().map(|node| node.msg_hash.clone()).collect()
    }

    #[test]
    fn linear_chain_heads_and_order() {
        let a = node(b"A", &[], 10);
        let b = node(b"B", &[b"A"], 20);
        let c = node(b"C", &[b"B"], 30);
        let nodes = vec![a, b, c];

        assert_eq!(forward_extremities(&nodes), vec![b"C".to_vec()]);
        assert_eq!(
            hashes(&deterministic_order(&nodes)),
            vec![b"A".to_vec(), b"B".to_vec(), b"C".to_vec()]
        );
    }

    #[test]
    fn branch_merge_heads_and_order() {
        let a = node(b"A", &[], 10);
        let b = node(b"B", &[b"A"], 30);
        let c = node(b"C", &[b"A"], 30);
        let merged = node(b"D", &[b"B", b"C"], 40);

        let branched = vec![a.clone(), b.clone(), c.clone()];
        assert_eq!(forward_extremities(&branched), vec![b"B".to_vec(), b"C".to_vec()]);

        let nodes = vec![a, c, b, merged];
        let ordered = deterministic_order(&nodes);
        assert_eq!(forward_extremities(&nodes), vec![b"D".to_vec()]);
        assert_eq!(
            hashes(&ordered),
            vec![b"A".to_vec(), b"B".to_vec(), b"C".to_vec(), b"D".to_vec()]
        );
        assert_eq!(ordered[3].depth, 2);
    }

    #[test]
    fn deterministic_order_is_independent_of_insertion_order() {
        let nodes_a = vec![
            node(b"A", &[], 10),
            node(b"B", &[b"A"], 30),
            node(b"C", &[b"A"], 30),
            node(b"D", &[b"B", b"C"], 40),
        ];
        let nodes_b = vec![
            node(b"D", &[b"B", b"C"], 40),
            node(b"C", &[b"A"], 30),
            node(b"A", &[], 10),
            node(b"B", &[b"A"], 30),
        ];

        assert_eq!(deterministic_order(&nodes_a), deterministic_order(&nodes_b));
    }

    #[test]
    fn outlier_changes_after_parent_arrives() {
        let child = node(b"B", &[b"A"], 20);
        let mut known = BTreeSet::new();
        assert!(is_outlier(&child, &known));
        assert_eq!(missing_parents(&child, &known), vec![b"A".to_vec()]);

        known.insert(b"A".to_vec());
        assert!(!is_outlier(&child, &known));
        assert!(missing_parents(&child, &known).is_empty());
    }

    #[test]
    fn missing_parents_for_batch_is_deduplicated_and_sorted() {
        let known = BTreeSet::from([b"A".to_vec()]);
        let nodes = vec![
            node(b"B", &[b"A", b"X"], 20),
            node(b"C", &[b"Y", b"X"], 21),
        ];

        assert_eq!(
            missing_parents_for_nodes(&nodes, &known),
            vec![b"X".to_vec(), b"Y".to_vec()]
        );
    }
}
