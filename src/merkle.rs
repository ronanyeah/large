use blake2::{Blake2b, Digest};
use serde::{Deserialize, Serialize};

pub type Hash = [u8; 32];

pub type Proof = Vec<Hash>;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MerkleTree {
    pub root: Hash,
    pub leaf_count: u32,
    levels: Vec<Vec<Hash>>,
}

impl MerkleTree {
    pub fn new(leaves: &[Hash]) -> anyhow::Result<Self> {
        if leaves.len() < 2 {
            return Err(anyhow::anyhow!("insufficient leaves"));
        }

        let mut levels = vec![leaves.to_vec()];
        let mut current_layer = leaves.to_vec();
        let leaf_count = leaves.len() as u32;

        // Build the tree level by level
        while current_layer.len() > 1 {
            current_layer = current_layer
                .chunks(2)
                .map(|pair| {
                    let left = pair[0];
                    let right = pair.get(1).copied().unwrap_or(left);
                    hash_pair(&left, &right)
                })
                .collect();
            levels.push(current_layer.clone());
        }

        Ok(MerkleTree {
            root: current_layer[0], // Last layer has single root node
            levels,
            leaf_count,
        })
    }

    pub fn get_root(&self) -> Hash {
        self.root
    }

    pub fn get_proof(&self, leaf: &Hash) -> (u64, Proof) {
        let leaf_index = self.get_leaf_index(leaf).expect("leaf not found");
        let mut proof = Vec::new();
        let mut index = leaf_index as usize;

        for current_level in self.levels.iter().take(self.levels.len() - 1) {
            let sibling_index = if index % 2 == 0 { index + 1 } else { index - 1 };
            let sibling = if sibling_index < current_level.len() {
                current_level[sibling_index]
            } else {
                current_level[index] // Duplicate the node itself if no sibling
            };
            proof.push(sibling);
            index /= 2;
        }

        (leaf_index, proof)
    }

    pub fn verify_proof(&self, leaf: &Hash, proof: &Proof) -> bool {
        let leaf_idx = self.get_leaf_index(leaf).expect("leaf not found");
        verify_proof(&self.root, leaf, proof, leaf_idx)
    }

    pub fn get_leaf_index(&self, leaf_hash: &Hash) -> Option<u64> {
        self.levels
            .first()?
            .iter()
            .position(|&hash| hash == *leaf_hash)
            .map(|x| x as u64)
    }
}

fn hash_pair(left: &Hash, right: &Hash) -> Hash {
    let mut hasher = Blake2b::new();
    hasher.update(left);
    hasher.update(right);
    hasher.finalize().into()
}

pub fn verify_proof(root: &Hash, leaf: &Hash, proof: &Proof, leaf_idx: u64) -> bool {
    let mut current_hash = *leaf;
    let mut current_idx = leaf_idx;

    // Iterate through the proof, hashing with each sibling
    for sibling in proof {
        // Determine if the current hash is left or right based on index
        current_hash = if current_idx % 2 == 0 {
            // Even index: current_hash is left, sibling is right
            hash_pair(&current_hash, sibling)
        } else {
            // Odd index: sibling is left, current_hash is right
            hash_pair(sibling, &current_hash)
        };

        // Move to the parent index
        current_idx /= 2;
    }

    // Final hash should match the global root
    current_hash == *root
}

#[cfg(test)]
mod tests {
    use super::*;
    use blake2::{Blake2b, Digest};
    use proptest::prelude::*;

    fn create_hash(data: &[u8]) -> Hash {
        let mut hasher = Blake2b::new();
        hasher.update(data);
        hasher.finalize().into()
    }

    // Strategy to generate random Hash values
    fn arb_hash() -> impl Strategy<Value = Hash> {
        prop::collection::vec(any::<u8>(), 32..=32).prop_map(|vec| {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&vec);
            arr
        })
    }

    // Strategy to generate a vector of thousands of leaves
    fn arb_leaves() -> impl Strategy<Value = Vec<Hash>> {
        prop::collection::vec(arb_hash(), 100..=1000)
    }

    #[test]
    fn test_new_merkle_tree_two_leaves() {
        let leaf1 = create_hash(b"leaf1");
        let leaf2 = create_hash(b"leaf2");
        let tree = MerkleTree::new(&[leaf1, leaf2]).unwrap();
        let expected_root = hash_pair(&leaf1, &leaf2);
        assert_eq!(tree.get_root(), expected_root);
        assert_eq!(tree.leaf_count, 2);
        assert_eq!(tree.levels.len(), 2);
        assert_eq!(tree.levels[0], vec![leaf1, leaf2]);
        assert_eq!(tree.levels[1], vec![expected_root]);
    }

    #[test]
    fn test_new_merkle_tree_odd_leaves() {
        let leaf1 = create_hash(b"leaf1");
        let leaf2 = create_hash(b"leaf2");
        let leaf3 = create_hash(b"leaf3");
        let tree = MerkleTree::new(&[leaf1, leaf2, leaf3]).unwrap();
        let pair1 = hash_pair(&leaf1, &leaf2);
        let pair2 = hash_pair(&leaf3, &leaf3); // Duplicate leaf3
        let root = hash_pair(&pair1, &pair2);
        assert_eq!(tree.get_root(), root);
        assert_eq!(tree.leaf_count, 3);
        assert_eq!(tree.levels.len(), 3);
        assert_eq!(tree.levels[0], vec![leaf1, leaf2, leaf3]);
        assert_eq!(tree.levels[1], vec![pair1, pair2]);
        assert_eq!(tree.levels[2], vec![root]);
    }

    #[test]
    #[should_panic(expected = "insufficient leaves")]
    fn test_new_merkle_tree_empty() {
        MerkleTree::new(&[]).unwrap();
    }

    #[test]
    #[should_panic]
    fn test_new_merkle_tree_single() {
        MerkleTree::new(&[create_hash(b"leaf1")]).unwrap();
    }

    #[test]
    fn test_get_proof_and_verify() {
        let leaf1 = create_hash(b"leaf1");
        let leaf2 = create_hash(b"leaf2");
        let leaf3 = create_hash(b"leaf3");
        let leaf4 = create_hash(b"leaf4");
        let tree = MerkleTree::new(&[leaf1, leaf2, leaf3, leaf4]).unwrap();

        let (index, proof) = tree.get_proof(&leaf1);
        assert_eq!(index, 0);
        assert_eq!(proof.len(), 2); // Two levels above leaves
        assert!(tree.verify_proof(&leaf1, &proof));

        let (index, proof) = tree.get_proof(&leaf3);
        assert_eq!(index, 2);
        assert_eq!(proof.len(), 2);
        assert!(tree.verify_proof(&leaf3, &proof));
    }

    #[test]
    fn test_verify_invalid_proof() {
        let leaf1 = create_hash(b"leaf1");
        let leaf2 = create_hash(b"leaf2");
        let leaf3 = create_hash(b"leaf3");
        let tree = MerkleTree::new(&[leaf1, leaf2, leaf3]).unwrap();

        let (_, mut proof) = tree.get_proof(&leaf1);
        // Tamper with the proof
        proof[0] = create_hash(b"invalid");
        assert!(!tree.verify_proof(&leaf1, &proof));
    }

    #[test]
    fn test_get_leaf_index() {
        let leaf1 = create_hash(b"leaf1");
        let leaf2 = create_hash(b"leaf2");
        let tree = MerkleTree::new(&[leaf1, leaf2]).unwrap();
        assert_eq!(tree.get_leaf_index(&leaf1), Some(0));
        assert_eq!(tree.get_leaf_index(&leaf2), Some(1));
        assert_eq!(tree.get_leaf_index(&create_hash(b"nonexistent")), None);
    }

    #[test]
    fn test_serialization() {
        let leaf1 = create_hash(b"leaf1");
        let leaf2 = create_hash(b"leaf2");
        let tree = MerkleTree::new(&[leaf1, leaf2]).unwrap();

        let serialized = serde_json::to_string(&tree).unwrap();
        let deserialized: MerkleTree = serde_json::from_str(&serialized).unwrap();

        assert_eq!(tree.get_root(), deserialized.get_root());
        assert_eq!(tree.leaf_count, deserialized.leaf_count);
        assert_eq!(tree.levels, deserialized.levels);
    }

    fn test_merkle_tree_large_leaves_impl(leaves: Vec<Hash>) -> Result<(), TestCaseError> {
        // Create MerkleTree with thousands of leaves
        let tree = MerkleTree::new(&leaves).unwrap();

        // Verify tree properties
        prop_assert_eq!(tree.leaf_count, leaves.len() as u32);
        prop_assert!(!tree.levels.is_empty());
        prop_assert_eq!(&tree.levels[0], &leaves.clone());
        prop_assert_eq!(tree.levels.last().unwrap().len(), 1); // Root level has one node
        prop_assert_eq!(tree.get_root(), tree.levels.last().unwrap()[0]);

        // Test proof generation and verification for a random leaf
        let random_leaf_index = leaves.len() / 2; // Pick a leaf from the middle
        let random_leaf = leaves[random_leaf_index];
        let (index, proof) = tree.get_proof(&random_leaf);
        prop_assert_eq!(index as usize, random_leaf_index);
        prop_assert!(tree.verify_proof(&random_leaf, &proof));

        // Test leaf index lookup
        prop_assert_eq!(
            tree.get_leaf_index(&random_leaf),
            Some(random_leaf_index as u64)
        );

        // Test invalid proof
        let mut tampered_proof = proof.clone();
        if !tampered_proof.is_empty() {
            tampered_proof[0] = create_hash(b"invalid");
            prop_assert!(!tree.verify_proof(&random_leaf, &tampered_proof));
        }
        Ok(())
    }

    proptest! {
        #[test]
        fn test_merkle_tree_large_leaves(leaves in arb_leaves()) {
            test_merkle_tree_large_leaves_impl(leaves)?;
        }
    }
}
