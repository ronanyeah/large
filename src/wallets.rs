use crate::merkle::Hash;
use anyhow::{anyhow, Context};
use blake2::Digest;
use csv::ReaderBuilder;
use std::collections::HashSet;
use std::fs::File;
use sui_sdk_types::Address;

pub fn parse_csv<R: std::io::Read>(reader: R) -> anyhow::Result<Vec<(Address, u64)>> {
    let mut rdr = ReaderBuilder::new().has_headers(false).from_reader(reader);

    let mut results = Vec::new();

    for result in rdr.records() {
        let record = result.context("Failed to parse CSV record")?;

        // Ensure the record has exactly 2 fields
        if record.len() != 2 {
            return Err(anyhow!("Invalid record format: {:?}", record));
        }

        let address = record[0].parse()?;
        let balance = record[1].parse()?;

        results.push((address, balance));
    }

    Ok(results)
}

pub fn read_wallets_csv(path: &str) -> anyhow::Result<Vec<(Address, u64)>> {
    let file = File::open(path).context("Failed to open CSV file")?;
    parse_csv(file)
}

pub fn parse_csv_bytes(data: &[u8]) -> anyhow::Result<Vec<(Address, u64)>> {
    let cursor = std::io::Cursor::new(data);
    parse_csv(cursor)
}

pub fn write_wallets_to_bytes(data: &Vec<(Address, u64)>) -> anyhow::Result<Vec<u8>> {
    let mut buffer = Vec::new();
    {
        let mut wtr = csv::WriterBuilder::new()
            .has_headers(false)
            .from_writer(&mut buffer);

        for (address, balance) in data {
            wtr.write_record(&[address.to_string(), balance.to_string()])
                .context("Failed to write CSV record")?;
        }

        wtr.flush().context("Failed to flush CSV writer")?;
    }

    Ok(buffer)
}

pub fn clean_addresses(
    mut addresses: Vec<(Address, u64)>,
) -> anyhow::Result<(u64, Vec<(Address, u64)>)> {
    {
        let addrs: HashSet<_> = addresses.iter().map(|(addr, _)| addr).collect();

        if addrs.len() < addresses.len() {
            return Err(anyhow!("duplicates"));
        }
    }

    {
        let any_empty = addresses.iter().any(|v| v.1 == 0);
        if any_empty {
            return Err(anyhow!("empty claim"));
        }
    }

    let total: u64 = addresses.iter().map(|(_, allo)| allo).sum();

    addresses.sort_by_key(|v| v.0);

    Ok((total, addresses))
}

pub fn hash_allo(address: &Address, allo: u64) -> Hash {
    let mut hasher = blake2::Blake2b::new();
    hasher.update(bcs::to_bytes(address).expect("u64 address fail"));
    hasher.update(bcs::to_bytes(&allo).expect("u64 bcs fail"));
    hasher.finalize().into()
}
