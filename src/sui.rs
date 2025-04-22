use anyhow::anyhow;
use std::str::FromStr;
use sui_sdk_types::{Address, ObjectId, TypeTag};
use sui_transaction_builder::{unresolved::Input, TransactionBuilder};

pub async fn create_tx(
    client: &sui_graphql_client::Client,
    sender: &Address,
) -> anyhow::Result<TransactionBuilder> {
    let mut builder = TransactionBuilder::new();

    let budget = 6_000_000;

    let sui_coin = crate::sui_coin().to_string();
    let gas_objs = client
        .coins(
            *sender,
            Some(&sui_coin),
            sui_graphql_client::PaginationFilter {
                direction: sui_graphql_client::Direction::Forward,
                cursor: None,
                limit: Some(10),
            },
        )
        .await?;

    let gas = gas_objs
        .data()
        .iter()
        .find(|obj| obj.balance() >= budget)
        .ok_or(anyhow!("no gas found"))?;

    let owned_obj = get_owned_obj(client, gas.id()).await?;

    builder.set_sender(*sender);
    builder.add_gas_objects(vec![owned_obj]);

    builder.set_gas_budget(budget);
    builder.set_gas_price(1_000);

    Ok(builder)
}

pub async fn fetch_type_param(
    client: &sui_graphql_client::Client,
    object: &ObjectId,
) -> anyhow::Result<TypeTag> {
    let obj = client
        .object((*object).into(), None)
        .await?
        .ok_or(anyhow!("drop not found"))?;
    if let sui_sdk_types::ObjectData::Struct(data) = obj.data() {
        let types = &data.object_type().type_params;
        let val = types.first().ok_or(anyhow!("type not found"))?;
        let tt = TypeTag::from_str(&val.to_string())?;
        return Ok(tt);
    }
    Err(anyhow!("type not found"))
}

pub async fn get_owned_obj(
    client: &sui_graphql_client::Client,
    obj: &ObjectId,
) -> anyhow::Result<Input> {
    let data = client
        .object((*obj).into(), None)
        .await?
        .ok_or(anyhow!("object not found"))?;

    let val = Input::by_id(*obj)
        .with_owned_kind()
        .with_version(data.version())
        .with_digest(data.digest());

    Ok(val)
}

pub async fn get_shared_obj(
    client: &sui_graphql_client::Client,
    obj: &ObjectId,
    mutable: bool,
) -> anyhow::Result<Input> {
    let data = client
        .object((*obj).into(), None)
        .await?
        .ok_or(anyhow!("object not found"))?;

    let sui_sdk_types::Owner::Shared(version) = data.owner() else {
        return Err(anyhow!("not shared obj"));
    };

    let val = Input::shared(*obj, *version, mutable);

    Ok(val)
}

pub fn find_created_obj(tx: &sui_sdk_types::TransactionEffects) -> anyhow::Result<ObjectId> {
    if let sui_sdk_types::TransactionEffects::V2(data) = tx {
        let obj = data.changed_objects.iter().find(|x| {
            x.output_state != sui_sdk_types::ObjectOut::NotExist
                && x.id_operation == sui_sdk_types::IdOperation::Created
        });
        if let Some(val) = obj {
            return Ok(val.object_id);
        }
    }
    Err(anyhow!("obj not found"))
}

pub async fn get_blob_from_obj(
    client: &sui_graphql_client::Client,
    id: &Address,
) -> anyhow::Result<String> {
    let obj = client
        .move_object_contents(*id, None)
        .await?
        .ok_or(anyhow!("object not found"))?;
    let val = (|| obj.as_object()?.get("blob_id")?.as_str())().ok_or(anyhow!("malformed json"))?;
    let n = crate::ffi::parse_u256_blob_id(val)?;
    Ok(n)
}

pub fn parse_address<'de, D>(deserializer: D) -> Result<Address, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value: [u8; 32] = serde::Deserialize::deserialize(deserializer)?;
    let addr = Address::from_bytes(value).expect("bad addr");

    Ok(addr)
}

pub fn suiprivkey_from_bytes(privkey: &[u8; 32]) -> anyhow::Result<String> {
    // Create 33-byte array: flag (0x00) + 32-byte private key
    let mut data = vec![0x00u8];
    data.extend_from_slice(privkey);

    let hrp = bech32::Hrp::parse("suiprivkey")?;

    let encoded = bech32::encode::<bech32::Bech32>(hrp, &data)?;
    Ok(encoded)
}
