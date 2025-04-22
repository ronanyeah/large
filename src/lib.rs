pub mod ffi;
pub mod merkle;
pub mod sui;
pub mod txns;
pub mod wallets;

use base64::Engine;
use std::str::FromStr;
use sui_sdk_types::Address;

pub fn sui_coin() -> sui_sdk_types::TypeTag {
    sui_sdk_types::TypeTag::from_str("0x2::coin::Coin<0x2::sui::SUI>").unwrap()
}

pub fn package_id() -> sui_sdk_types::Address {
    // TESTNET
    sui_sdk_types::Address::from_str(
        "0x5cccbfec0ef491993f5b2aa19b98845476ad720254c2e758254e23dbe547b94d",
    )
    .unwrap()
}

pub fn drop_object() -> sui_sdk_types::ObjectId {
    // A pre-created testnet campaign
    sui_sdk_types::ObjectId::from_str(
        "0xdda2402ee7e7a4cb0a5a68692e9dac087be029bbd7d518e189121387a12b71b1",
    )
    .unwrap()
}

pub async fn fetch_merkle_tree(
    client: &sui_graphql_client::Client,
    object: &Address,
) -> anyhow::Result<merkle::MerkleTree> {
    let blob_id = sui::get_blob_from_obj(client, object).await?;
    let data = ffi::read_blob(&blob_id).await?;
    let bts = base64::engine::general_purpose::STANDARD.decode(data.blob)?;
    let out = bcs::from_bytes(&bts)?;
    Ok(out)
}

pub async fn fetch_allocations(
    client: &sui_graphql_client::Client,
    object: &Address,
) -> anyhow::Result<Vec<(Address, u64)>> {
    let blob_id = sui::get_blob_from_obj(client, object).await?;
    let data = ffi::read_blob(&blob_id).await?;
    let bts = base64::engine::general_purpose::STANDARD.decode(data.blob)?;
    let out = wallets::parse_csv_bytes(&bts)?;
    Ok(out)
}

//type Allocations = Vec<(Address, u64)>;

pub trait AllocationExt {
    fn get_allocation(&self, wallet: &Address) -> Option<u64>;
}

impl AllocationExt for Vec<(Address, u64)> {
    fn get_allocation(&self, wallet: &Address) -> Option<u64> {
        self.iter().find(|(addr, _)| addr == wallet).map(|v| v.1)
    }
}
