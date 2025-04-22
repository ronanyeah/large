use anyhow::anyhow;
use base64::Engine;
use std::io::Write;
use sui_sdk_types::Address;
use tempfile::NamedTempFile;
use tokio::process::Command;
use url::Url;

pub async fn sui_check() -> anyhow::Result<String> {
    let err_msg =
        "No Sui CLI found. More: https://docs.sui.io/guides/developer/getting-started/sui-install";
    let output = Command::new("sui")
        .arg("--version")
        .output()
        .await
        .map_err(|_| anyhow!(err_msg))?;
    let out = parse_terminal_output(&output)?;
    Ok(out)
}

pub async fn walrus_check() -> anyhow::Result<String> {
    let err_msg = "No Walrus CLI found. More: https://docs.wal.app/usage/setup.html";
    let output = Command::new("walrus")
        .arg("--version")
        .output()
        .await
        .map_err(|_| anyhow!(err_msg))?;
    let out = parse_terminal_output(&output)?;
    Ok(out)
}

pub async fn env_check() -> anyhow::Result<(String, String)> {
    let res = futures::future::try_join(sui_check(), walrus_check()).await?;

    Ok(res)
}

pub async fn current_rpc() -> anyhow::Result<Url> {
    let output = Command::new("sui")
        .arg("client")
        .arg("envs")
        .arg("--json")
        .output()
        .await?;

    let json_str = parse_terminal_output(&output)?;

    let res = serde_json::from_str(&json_str)?;

    let rpc = get_active_rpc(res)?;

    Ok(rpc)
}

pub async fn current_wallet() -> anyhow::Result<Address> {
    let output = Command::new("sui")
        .arg("client")
        .arg("active-address")
        .arg("--json")
        .output()
        .await?;

    let json_str = parse_terminal_output(&output)?;

    let res = serde_json::from_str(&json_str)?;
    Ok(res)
}

pub async fn sign_tx(
    wallet: &Address,
    tx: &sui_sdk_types::Transaction,
) -> anyhow::Result<sui_sdk_types::UserSignature> {
    #[derive(Debug, serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct SignedTx {
        sui_signature: String,
    }

    let tx_data = base64::engine::general_purpose::STANDARD.encode(&bcs::to_bytes(&tx)?);

    let output = Command::new("sui")
        .arg("keytool")
        .arg("sign")
        .args(["--address", &wallet.to_string()])
        .args(["--data", &tx_data])
        .arg("--json")
        .output()
        .await?;

    let json_str = parse_terminal_output(&output)?;

    let res: SignedTx = serde_json::from_str(&json_str)?;
    let sig = sui_sdk_types::UserSignature::from_base64(&res.sui_signature)?;
    Ok(sig)
}

pub async fn write_files(files: Vec<String>, epochs: u32) -> anyhow::Result<Vec<NewBlob>> {
    let file_paths = files
        .iter()
        .map(|f| format!("\"{}\"", f))
        .collect::<Vec<_>>()
        .join(", ");

    let json_input = format!(
        r#"
        {{
            "command": {{
                "store": {{
                    "files": [{}],
                    "epochs": {},
                    "deletable": true
                }}
            }}
        }}
        "#,
        file_paths, epochs
    );

    let output = Command::new("walrus")
        .arg("json")
        .arg(&json_input)
        .arg("--json")
        .output()
        .await
        .map_err(|e| anyhow!("Failed to execute walrus command: {}", e))?;

    let json_str = parse_terminal_output(&output)?;

    let mut json = serde_json::from_str::<Vec<BlobStoreResult>>(&json_str)?;

    // Ensure blobs are in correct order
    reorder_results(&mut json, &files);

    let blobs = json
        .into_iter()
        .map(|v| {
            Ok(NewBlob {
                blob_id: v.blob_store_result.newly_created.blob_object.blob_id,
                object_address: v.blob_store_result.newly_created.blob_object.id.parse()?,
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(blobs)
}

pub async fn write_blobs(values: Vec<&[u8]>, epochs: u32) -> anyhow::Result<Vec<NewBlob>> {
    // Keep temp_file in scope to prevent deletion
    let mut temp_files: Vec<NamedTempFile> = Vec::new();
    let mut temp_file_paths: Vec<String> = Vec::new();

    for value in values {
        let mut temp_file = NamedTempFile::new()?;
        temp_file
            .write_all(value)
            .map_err(|e| anyhow!("Failed to write to temp file: {}", e))?;
        temp_file
            .flush()
            .map_err(|e| anyhow!("Failed to flush temp file: {}", e))?;

        let temp_file_path = temp_file
            .path()
            .to_str()
            .ok_or_else(|| anyhow!("Invalid temp file path"))?
            .to_string();

        temp_file_paths.push(temp_file_path);
        temp_files.push(temp_file);
    }

    let blob_ids = write_files(temp_file_paths, epochs).await?;

    Ok(blob_ids)
}

pub fn parse_u256_blob_id(id: &str) -> anyhow::Result<String> {
    let n = primitive_types::U256::from_dec_str(id)?;
    let val = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(n.to_little_endian());
    Ok(val)
}

pub async fn read_blob(id: &str) -> anyhow::Result<Blob> {
    let json_input = format!(
        r#"
        {{
            "command": {{
                "read": {{
                    "blobId": "{}"
                }}
            }}
        }}
    "#,
        id
    );

    let output = Command::new("walrus")
        .arg("json")
        .arg(json_input)
        .arg("--json")
        .output()
        .await?;

    let json_str = parse_terminal_output(&output)?;

    let json = serde_json::from_str::<Blob>(&json_str)?;

    Ok(json)
}

fn reorder_results(results: &mut [BlobStoreResult], paths: &[String]) {
    results.sort_by(|a, b| {
        let index_a = paths.iter().position(|p| p == &a.path).unwrap();
        let index_b = paths.iter().position(|p| p == &b.path).unwrap();
        index_a.cmp(&index_b)
    });
}

#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct Blob {
    pub blob: String,
    pub blob_id: String,
}

#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct NewBlob {
    pub blob_id: String,
    pub object_address: Address,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct BlobStoreResult {
    blob_store_result: NewlyCreatedWrapper,
    path: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewlyCreatedWrapper {
    newly_created: NewlyCreated,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct NewlyCreated {
    blob_object: BlobObject,
    cost: u64,
    resource_operation: ResourceOperation,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct BlobObject {
    blob_id: String,
    certified_epoch: u32,
    deletable: bool,
    encoding_type: String,
    id: String,
    registered_epoch: u32,
    size: u64,
    storage: Storage,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct Storage {
    end_epoch: u32,
    id: String,
    start_epoch: u32,
    storage_size: u64,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct ResourceOperation {
    register_from_scratch: RegisterFromScratch,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct RegisterFromScratch {
    encoded_length: u64,
    epochs_ahead: u32,
}

fn get_active_rpc(json: serde_json::Value) -> anyhow::Result<Url> {
    #[derive(serde::Deserialize)]
    struct NetworkConfig {
        alias: String,
        rpc: String,
        //ws: Option<String>,
        //basic_auth: Option<String>,
    }

    let (networks, active_alias): (Vec<NetworkConfig>, String) = serde_json::from_value(json)?;

    // Find the network with the matching alias
    let active_network = networks
        .into_iter()
        .find(|network| network.alias == active_alias)
        .ok_or_else(|| anyhow!("No network found with alias: {}", active_alias))?;

    let rpc_url =
        Url::parse(&active_network.rpc).map_err(|e| anyhow!("Failed to parse RPC URL: {}", e))?;

    Ok(rpc_url)
}

fn parse_terminal_output(output: &std::process::Output) -> anyhow::Result<String> {
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        return Err(anyhow!(stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();

    let out = stdout.trim();

    Ok(out.to_string())
}
