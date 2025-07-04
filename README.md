# Large Protocol

[large.fun](https://large.fun/)

---

Large Protocol provides the ability to cheaply create decentralized airdrop campaigns for millions of users.

Wallet and allocation data is stored offchain using Walrus, and claims are verified on the Sui blockchain.

Large is currently live on Sui Mainnet.

[Program](https://suivision.xyz/package/0x7daa0be72e7c857f40f8aefa74ac835dccfa4e25ec40a92665e54249d93d3892)

### CLI

This repository privides a Rust CLI which can be use to create campaigns, generate proofs and execute claims.

It requires both the [Sui CLI](https://docs.sui.io/guides/developer/getting-started/sui-install) and [Walrus CLI](https://docs.wal.app/usage/setup.html) to be installed.

#### > Install

`cargo install --locked --git https://github.com/ronanyeah/large.git`


```
Usage: large <COMMAND>

Commands:
  create-drop     Create a new airdrop campaign
  current-wallet  Print currently active wallet in Sui CLI
  claim           Execute a claim with current wallet
  check-claim     Check any address for claim amount
  check-env       Check that Sui + Walrus CLIs are installed
  help            Print this message or the help of the given subcommand(s)
```

### Demo

A Sui testnet campaign with 1 million Sui wallets included in the airdrop.

#### > Walrus data
- [Wallets + token allocations](https://walruscan.com/testnet/blob/hAsa4S6oysAexymYeF555YFAzcJq5TyXeQPzeDjjhHs) [CSV file - 80mb]
- [Merkle tree](https://walruscan.com/testnet/blob/clO5fSMCMPp2Cc6D0_uAtfblm_-B8ItVqovHUAxPxzg) [BCS encoded - 64mb]

#### > Onchain

- [Campaign object](https://testnet.suivision.xyz/object/0xdda2402ee7e7a4cb0a5a68692e9dac087be029bbd7d518e189121387a12b71b1)

#### > Claim transactions

- [Wallet index #0](https://testnet.suivision.xyz/txblock/AcXJ9gPGLQLFPkvBivHaXTwjjS9AAigytWQDWb6KwRZg)
- [Wallet index #500000](https://testnet.suivision.xyz/txblock/46bxmHVsXNHLSXhLdf5ex77uXKcNuLgSeA1mBXC7gtdQ)
- [Wallet index #999999](https://testnet.suivision.xyz/txblock/9Dobttit4pWxxWs2Dj9c5JqW38WE3Btqw2L9ZfKwrshH)


### Development

- [x] Sui program
- [x] CLI tool
- [ ] Cache server
- [ ] SDKs (TypeScript, Rust)
- [ ] Claim frontend
- [ ] MCP server
- [ ] Snapshot tool
- [ ] ???