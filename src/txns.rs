use crate::{
    merkle, package_id,
    sui::{create_tx, get_owned_obj, get_shared_obj, parse_address},
};
use anyhow::anyhow;
use sui_sdk_types::{Address, Identifier, ObjectId, Transaction, TypeTag};
use sui_transaction_builder::Serialized;

#[derive(serde::Deserialize, Debug, serde::Serialize)]
pub struct Table {
    pub id: [u8; 32],
    pub size: u64,
}

#[derive(serde::Deserialize, Debug, serde::Serialize)]
pub struct DeleteCap {
    pub id: [u8; 32],
    pub object_id: [u8; 32],
}

#[derive(serde::Deserialize, Debug, serde::Serialize)]
pub struct Drop {
    pub id: [u8; 32],
    pub root: Vec<u8>,
    pub wallet_count: u32,
    pub airdrop_total: u64,
    pub vault: u64,
    #[serde(deserialize_with = "parse_address")]
    pub allocations: Address,
    #[serde(deserialize_with = "parse_address")]
    pub merkle_tree: Address,
    pub registry: Table,
}

pub async fn create_claim_tx(
    client: &sui_graphql_client::Client,
    sender: &Address,
    proof: &merkle::Proof,
    leaf_index: u64,
    drop_id: &ObjectId,
    coin_type: &TypeTag,
    allo: u64,
) -> anyhow::Result<Transaction> {
    let mut builder = create_tx(client, sender).await?;

    let func = sui_transaction_builder::Function::new(
        package_id(),
        Identifier::new("drop")?,
        Identifier::new("claim")?,
        vec![coin_type.clone()],
    );

    let proof_data: Vec<_> = proof.iter().map(|x| x.to_vec()).collect();

    let drop_obj = get_shared_obj(client, drop_id, true).await?;

    let sender_arg = builder.input(Serialized(&sender));

    let arg0 = builder.input(Serialized(&proof_data));
    let arg1 = builder.input(Serialized(&leaf_index));
    let arg2 = builder.input(Serialized(&allo));
    let arg3 = builder.input(drop_obj);
    let coins = builder.move_call(func, vec![arg0, arg1, arg2, arg3]);
    builder.transfer_objects(vec![coins], sender_arg);

    let tx = builder.finish()?;

    Ok(tx)
}

pub async fn create_drop_tx(
    client: &sui_graphql_client::Client,
    sender: &Address,
    walrus_addresses: &Address,
    walrus_merkle: &Address,
    funds: u64,
    wallet_count: u32,
    coin_type: &TypeTag,
    merkle_root: &merkle::Hash,
) -> anyhow::Result<Transaction> {
    let mut builder = create_tx(client, sender).await?;

    let func = sui_transaction_builder::Function::new(
        package_id(),
        Identifier::new("drop")?,
        Identifier::new("create_drop")?,
        vec![coin_type.clone()],
    );

    let coin_tag = format!("0x2::coin::Coin<{}>", coin_type);
    let coin_objs = client
        .coins(
            *sender,
            Some(&coin_tag),
            sui_graphql_client::PaginationFilter {
                direction: sui_graphql_client::Direction::Forward,
                cursor: None,
                limit: Some(10),
            },
        )
        .await?;

    let target = coin_objs
        .data()
        .iter()
        .find(|obj| obj.balance() >= funds)
        .ok_or(anyhow!("no coin found"))?;

    let owned_coin = get_owned_obj(client, target.id()).await?;
    let sender_arg = builder.input(Serialized(&sender));
    let coin_arg = builder.input(owned_coin);
    let funds_arg = builder.input(Serialized(&funds));
    let coins = builder.split_coins(coin_arg, vec![funds_arg]);

    let arg0 = builder.input(Serialized(&merkle_root.to_vec()));
    let arg1 = coins.nested(0).ok_or(anyhow!("no coin split"))?;
    let arg2 = builder.input(Serialized(&walrus_addresses));
    let arg3 = builder.input(Serialized(&walrus_merkle));
    let arg4 = builder.input(Serialized(&wallet_count));

    let res = builder.move_call(func, vec![arg0, arg1, arg2, arg3, arg4]);
    builder.transfer_objects(vec![res], sender_arg);

    let tx = builder.finish()?;

    Ok(tx)
}

pub async fn delete_drop_tx(
    client: &sui_graphql_client::Client,
    sender: &Address,
    coin_type: &TypeTag,
    drop_id: &ObjectId,
    cap_id: &ObjectId,
) -> anyhow::Result<Transaction> {
    let mut builder = create_tx(client, sender).await?;

    let func = sui_transaction_builder::Function::new(
        package_id(),
        Identifier::new("drop")?,
        Identifier::new("destroy_drop")?,
        vec![coin_type.clone()],
    );

    let cap_obj = get_owned_obj(client, cap_id).await?;
    let drop_obj = get_shared_obj(client, drop_id, true).await?;

    let sender_arg = builder.input(Serialized(&sender));
    let arg0 = builder.input(cap_obj);
    let arg1 = builder.input(drop_obj);
    let coins = builder.move_call(func, vec![arg0, arg1]);
    builder.transfer_objects(vec![coins], sender_arg);

    let tx = builder.finish()?;

    Ok(tx)
}

pub async fn get_delete_cap(
    client: &sui_graphql_client::Client,
    sender: &Address,
    drop_id: &ObjectId,
) -> anyhow::Result<ObjectId> {
    let delete_cap_type = format!("{}::drop::DeleteCap", package_id());
    let delete_caps = client
        .objects(
            Some(sui_graphql_client::query_types::ObjectFilter {
                owner: Some(*sender),
                object_ids: None,
                type_: Some(&delete_cap_type),
            }),
            sui_graphql_client::PaginationFilter {
                direction: sui_graphql_client::Direction::Forward,
                cursor: None,
                limit: Some(10),
            },
        )
        .await?;

    for obj in delete_caps.data() {
        if let sui_sdk_types::ObjectData::Struct(data) = obj.data() {
            let gg: DeleteCap = bcs::from_bytes(data.contents())?;
            let id = ObjectId::from(gg.object_id);
            if *drop_id == id {
                return Ok(ObjectId::from(gg.id));
            }
        }
    }

    Err(anyhow!("DeleteCap not found"))
}
