module large::drop;

use sui::hash;
use sui::{coin, table::Table};

public struct Drop<phantom TOKEN> has key, store {
    id: UID,
    root: vector<u8>,
    wallet_count: u32,
    airdrop_total: u64,
    vault: sui::balance::Balance<TOKEN>,
    // Walrus object - addresses + allocations
    allocations: address,
    // Walrus object - tree
    merkle_tree: address,
    registry: Table<address, bool>,
}

public struct DeleteCap has key, store {
    id: UID,
    object_id: ID,
}

public fun create_drop<TOKEN>(
    root: vector<u8>,
    funds: coin::Coin<TOKEN>,
    leaves_storage: address,
    merkle_storage: address,
    wallet_count: u32,
    ctx: &mut TxContext,
): DeleteCap {
    assert!(wallet_count >= 2);
    assert!(root.length() == 32);
    assert!(funds.value() > 0);

    let id = object::new(ctx);
    let admin_cap = DeleteCap {
        id: object::new(ctx),
        object_id: object::uid_to_inner(&id),
    };
    let drop = Drop<TOKEN> {
        id,
        root,
        wallet_count,
        airdrop_total: funds.value(),
        allocations: leaves_storage,
        merkle_tree: merkle_storage,
        vault: funds.into_balance(),
        registry: sui::table::new(ctx),
    };
    transfer::public_share_object(drop);
    admin_cap
}

public fun destroy_drop<TOKEN>(
    cap: DeleteCap,
    drop: Drop<TOKEN>,
    ctx: &mut TxContext,
): coin::Coin<TOKEN> {
    assert!(object::uid_to_inner(&drop.id) == cap.object_id);
    let DeleteCap { id: cap_id, object_id: _ } = cap;
    object::delete(cap_id);
    let Drop {
        id,
        vault,
        registry,
        root: _,
        wallet_count: _,
        airdrop_total: _,
        allocations: _,
        merkle_tree: _,
    } = drop;
    registry.drop();
    object::delete(id);
    coin::from_balance(vault, ctx)
}

public fun claim<TOKEN>(
    proof: vector<vector<u8>>,
    leaf_index: u64,
    allocation: u64,
    drop: &mut Drop<TOKEN>,
    ctx: &mut TxContext,
): coin::Coin<TOKEN> {
    assert!(leaf_index < drop.wallet_count as u64);
    assert!(proof.length() == proof_length(drop.wallet_count as u64));

    let res = verify_sender_proof(drop.root, &proof, &ctx.sender(), allocation, leaf_index);

    assert!(res);

    drop.registry.add(ctx.sender(), true);

    let bal = drop.vault.split(allocation);
    coin::from_balance(bal, ctx)
}

public fun has_claimed<TOKEN>(addr: address, drop: &Drop<TOKEN>): bool {
    drop.registry.contains(addr)
}

/// internal

public(package) fun verify_sender_proof(
    root: vector<u8>,
    proof: &vector<vector<u8>>,
    sender: &address,
    allocation: u64,
    leaf_index: u64,
): bool {
    let leaf_bytes = hash_address_w_allocation(sender, allocation);

    verify_proof(proof, root, leaf_bytes, leaf_index)
}

fun verify_proof(
    proof: &vector<vector<u8>>,
    root: vector<u8>,
    leaf: vector<u8>,
    leaf_index: u64,
): bool {
    assert!(root.length() == 32);
    assert!(leaf.length() == 32);
    assert!(proof.all!(|h| h.length() == 32));

    let node_set = sui::vec_set::from_keys(*proof);
    assert!(node_set.size() == proof.length());

    let computed_hash = compute_proof(leaf, proof, leaf_index);
    computed_hash == root
}

fun compute_proof(
    leaf: vector<u8>,
    proof: &vector<vector<u8>>,
    mut current_index: u64,
): vector<u8> {
    let mut current_hash = leaf;
    let proof_length = proof.length();
    let mut i = 0;

    while (i < proof_length) {
        let sibling = *vector::borrow(proof, i);
        // Determine ordering based on index
        if (current_index % 2 == 0) {
            // Even index: current_hash is left, sibling is right
            current_hash = hash_slices(current_hash, sibling);
        } else {
            // Odd index: sibling is left, current_hash is right
            current_hash = hash_slices(sibling, current_hash);
        };
        // Move to parent index
        current_index = current_index / 2;
        i = i + 1;
    };

    current_hash
}

fun hash_slices(mut a: vector<u8>, b: vector<u8>): vector<u8> {
    vector::append(&mut a, b);
    hash::blake2b256(&a)
}

fun hash_address_w_allocation(addr: &address, allocation: u64): vector<u8> {
    let mut bts = vector::empty();
    vector::append(&mut bts, sui::bcs::to_bytes(addr));
    vector::append(&mut bts, sui::bcs::to_bytes(&allocation));
    hash::blake2b256(&bts)
}

fun proof_length(leaves: u64): u64 {
    let mut x = leaves - 1; // Adjust for ceiling: non-power-of-2 rounds up.
    let mut log = 0;

    while (x > 0) {
        x = x >> 1; // Divide by 2 via right shift.
        log = log + 1;
    };

    log
}
