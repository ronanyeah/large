use anyhow::Context;
use clap::{Parser, Subcommand};
use large::merkle::MerkleTree;
use large::sui;
use large::{drop_object, ffi, txns, wallets, AllocationExt};
use spinners::{Spinner, Spinners};
use std::str::FromStr;
use sui_sdk_types::{Address, ObjectId, TypeTag};

#[derive(Parser)]
#[command(
    version,
    about = "🖧  Large Protocol

Create low-cost airdrop campaigns to millions of users.

Currently live on Sui Testnet.

This tool requires the Sui and Walrus CLIs to be installed."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new airdrop campaign.
    CreateDrop {
        #[clap(
            help = "Path to the CSV file containing the wallet addresses and token allocations"
        )]
        path: String,
    },
    /// Print currently active wallet in Sui CLI.
    CurrentWallet,
    /// Execute a claim with current wallet.
    Claim {
        #[clap(help = "The object ID of the campaign you want to claim from")]
        drop_id: Option<ObjectId>,
    },
    /// Check any address for claim amount.
    CheckClaim {
        #[clap(
            help = "The wallet address to check for a claim. Defaults to active Sui CLI wallet"
        )]
        wallet: Option<Address>,
        #[clap(
            help = "The object ID of the campaign you want to check. Defaults to Testnet demo campaign"
        )]
        drop_id: Option<ObjectId>,
    },
    /// Check that Sui + Walrus CLIs are installed.
    CheckEnv,
}

const EPOCHS: u32 = 4;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let wallet_task = tokio::spawn(ffi::current_wallet());

    let client = sui_graphql_client::Client::new_testnet();

    let cli = Cli::parse();
    match cli.command {
        Commands::CreateDrop { path } => {
            ffi::env_check().await?;

            let data = wallets::read_wallets_csv(&path)?;
            let coin_txt = inquire::Text::new("What coin type do you want to airdrop?").prompt()?;
            let coin_type = TypeTag::from_str(&coin_txt)?;

            let (total, wallets) = wallets::clean_addresses(data)?;
            println!("Wallet count: {}", wallets.len());
            println!("Airdrop token total: {}", total);
            println!("Building merkle tree...");
            let merk = {
                let roots: Vec<_> = wallets
                    .iter()
                    .map(|(addr, allo)| wallets::hash_allo(addr, *allo))
                    .collect();
                MerkleTree::new(&roots)?
            };
            let merkle_bts = bcs::to_bytes(&merk)?;

            let top_root = merk.get_root();
            let address_bts = wallets::write_wallets_to_bytes(&wallets)?;

            println!("Writing to Walrus...");
            let blobs = ffi::write_blobs(vec![&merkle_bts, &address_bts], EPOCHS).await?;
            let merkle_addr = blobs.first().ok_or("missing merkle blob")?.object_address;
            let list_addr = blobs.get(1).ok_or("missing addresses blob")?.object_address;

            println!("Creating transaction...");
            let wallet = wallet_task.await??;
            let tx = txns::create_drop_tx(
                &client,
                &wallet,
                &list_addr,
                &merkle_addr,
                total,
                merk.leaf_count,
                &coin_type,
                &top_root,
            )
            .await?;

            println!("Signing transaction...");
            let sig = ffi::sign_tx(&wallet, &tx).await?;
            println!("Submitting transaction...");
            let res = client
                .execute_tx(vec![sig], &tx)
                .await?
                .ok_or("missing tx")?;

            println!("TX status: {:?}", res.status());
            println!("TX digest: {}", tx.digest());
            let new_campaign_id = sui::find_created_shared_obj(&res)?;
            println!("New campaign object ID: {new_campaign_id}");
        }
        Commands::CurrentWallet => {
            ffi::sui_check().await?;
            let wallet = wallet_task.await??;
            println!("Active wallet: {:?}", wallet);
        }
        Commands::Claim { drop_id } => {
            ffi::sui_check().await?;

            let wallet = wallet_task.await??;
            println!("Active wallet: {}", wallet);

            let drop_obj = drop_id.unwrap_or(drop_object());
            println!("Claiming from drop: {}", drop_obj);
            let obj = client
                .move_object_contents_bcs(drop_obj.into(), None)
                .await?
                .ok_or("drop not found")?;
            let tt = sui::fetch_type_param(&client, &drop_obj).await?;
            let data: txns::Drop = bcs::from_bytes(&obj)?;

            let start_time = std::time::Instant::now();
            let mut sp = Spinner::new(Spinners::Aesthetic, "Reading blobs...".into());
            let (merkle_tree, addresses) = futures::future::try_join(
                large::fetch_merkle_tree(&client, &data.merkle_tree),
                large::fetch_allocations(&client, &data.allocations),
            )
            .await?;
            let total_elapsed = start_time.elapsed().as_millis();
            sp.stop_with_message(format!("Done in {:.2}s", total_elapsed as f64 / 1000.0));

            let allo = addresses
                .get_allocation(&wallet)
                .ok_or("no allocation found")?;
            let leaf = wallets::hash_allo(&wallet, allo);

            let (leaf_index, proof) = merkle_tree.get_proof(&leaf);

            assert!(merkle_tree.verify_proof(&leaf, &proof), "Invalid proof");

            let tx =
                txns::create_claim_tx(&client, &wallet, &proof, leaf_index, &drop_obj, &tt, allo)
                    .await?;

            let sig = ffi::sign_tx(&wallet, &tx).await?;
            let res = client
                .execute_tx(vec![sig], &tx)
                .await?
                .ok_or("missing tx")?;

            println!("TX status: {:?}", res.status());
            println!("TX digest: {}", tx.digest());
        }
        Commands::CheckClaim { wallet, drop_id } => {
            let sender = wallet.unwrap_or(wallet_task.await??);
            let drop_obj_id = drop_id.unwrap_or(drop_object());
            println!("Checking claim in drop ID: {}", drop_obj_id);
            println!("Wallet selected: {}", sender);
            let drop_obj: txns::Drop = {
                let obj = client
                    .move_object_contents_bcs(drop_obj_id.into(), None)
                    .await?
                    .ok_or("drop not found")?;
                bcs::from_bytes(&obj).context("drop decode")?
            };

            let tt = sui::fetch_type_param(&client, &drop_obj_id).await?;

            let coin = client
                .coin_metadata(&tt.to_string())
                .await?
                .ok_or("coin not found")?;
            let decimals = coin.decimals.ok_or("unknown decimals")? as u32;
            let shift = 10_f64.powf(decimals as f64);

            let mut sp = Spinner::new(Spinners::Aesthetic, "Reading from Walrus...".into());
            let addresses = large::fetch_allocations(&client, &drop_obj.allocations).await?;
            sp.stop_with_newline();

            let allo = addresses.get_allocation(&sender).unwrap_or(0);

            println!("Allocation for wallet: {}", sender);
            println!(
                "{:.2} ${}",
                allo as f64 / shift,
                coin.symbol.unwrap_or("TOKEN".to_string())
            );
        }
        Commands::CheckEnv => {
            let (sui_version, walrus_version) = ffi::env_check().await?;
            println!("✅ Sui CLI: {}", sui_version);
            println!("✅ Walrus CLI: {}", walrus_version);
        }
    }

    Ok(())
}
