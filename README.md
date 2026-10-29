<div align="center">

# Zoner

**Foundry for Logos. Same Solidity, same tools, one new network.**

A small command-line companion that takes your Foundry project from `init` to a running, Logos-published chain in a few commands.

`Rust` · `Foundry compatible` · `Solizone EVM` · `Logos Zones` · `Chain ID 9001`

</div>

> Zoner and Solizone are independent projects and are **not official Logos projects**.

---

## Why Zoner exists

Logos gives builders strong privacy and a serious infrastructure stack. Most smart-contract developers, though, already live in Solidity and Foundry, and a new network that asks them to start over rarely gets built on.

Zoner removes that cost. It does not replace Foundry. It lets Foundry talk to [Solizone EVM](https://github.com/bristinWild/solizone), an EVM execution layer whose blocks are published to Logos.

```text
forge / cast / ethers / viem          <- you keep these
            |
          Zoner                       <- init, run a node, deploy, status, sync
            |
      Solizone EVM (REVM)             <- executes your Solidity
            |
   Logos Zone SDK + Logos Storage     <- ordering, finality, durable history
```

## What you get

| Command | What it does |
|---|---|
| `zoner init [DIR]` | Creates a Foundry project already pointed at Solizone (chain id 9001), with an example `Counter` contract, `.gitignore` and a short README. It never overwrites existing files. |
| `zoner node [flags]` | Starts a local Solizone node. Defaults to local storage, so you do not need Basecamp. Chain data goes in `~/.zoner`, not in your project folder. |
| `zoner deploy CONTRACT` | Deploys with `forge create` to the node, signed by a pre-funded dev account or your own key. Supports constructor arguments. |
| `zoner status` | Shows chain id, head block and Logos publication progress (next block, blocks waiting for finalization, failures, last five block statuses). |
| `zoner sync` | Rebuilds the whole chain **from Logos**, re-executes and verifies every block, then keeps running. |
| `zoner accounts` | Lists the five pre-funded dev accounts (public test keys). |

Everything else (`forge build`, `forge test`, `cast call`, `cast send`) is plain Foundry, and it works because Solizone speaks standard Ethereum JSON-RPC.

---

## Quick start

You need [Rust](https://rustup.rs) and [Foundry](https://getfoundry.sh).

```bash
# 1. install Zoner
git clone https://github.com/bristinWild/Zoner
cd Zoner
cargo install --path .

# 2. make a project
zoner init hello-logos
cd hello-logos

# 3. start a local chain (second terminal)
zoner node

# 4. deploy
zoner deploy src/Counter.sol:Counter

# 5. use it with the tools you know
cast send <deployed address> "increment()" \
  --rpc-url http://127.0.0.1:8545 --private-key <dev key from `zoner accounts`>

# 6. see what is happening
zoner status
```

`zoner node` runs the `solizone_node` binary from [bristinWild/solizone](https://github.com/bristinWild/solizone). Build it there (`cargo build --release --bin solizone_node`), then either put it next to the `zoner` binary, on your `PATH`, or point to it with `SOLIZONE_NODE=/path/to/solizone_node`.

### Publish to Logos

```bash
zoner node --publish
zoner status
```

```text
chain id:    9001
head block:  #12
publishing:  next block #13, 0 not yet submitted, 3 waiting for Logos finalization
  block #12     submitted
  block #11     submitted
  block #10     finalized
```

Every block is sent to a Logos zone channel in height order. A block counts as `finalized` once Logos finalizes it, which takes about 12 hours on the current network. That delay is how Logos finality works.

### Rebuild a chain from Logos

```bash
zoner node reset      # a sync needs an empty data directory
zoner sync
```

Zoner reads the published blocks back, replays each one through the EVM and checks every commitment. A node can recover from the network alone, with no trust in a server.

---

## Why this is interesting

**For Logos builders.** You get a familiar Solidity workflow on Logos in minutes, and the published blocks are real zone data that anyone can read back and verify.

**For the ecosystem.** Each new contract written with Zoner adds activity to the network. The tool lowers the first-hour cost for a whole group of developers who would otherwise stay on Ethereum.

**For investors and grant reviewers.**
- *Small surface, high leverage.* Zoner is deliberately thin (about 750 lines of Rust). It wraps Foundry instead of rebuilding it, so it stays easy to maintain and low-risk.
- *Clean boundary.* It talks to a node only through JSON-RPC and the node binary. It has no code dependency on the node crate, so the node can change without breaking the tool.
- *Part of a stack.* Zoner sits between [Solizone EVM](https://github.com/bristinWild/solizone) (the execution layer) and SoliCamp (a no-terminal Solidity playground inside Logos Basecamp). Together they cover the CLI developer, the browser-style developer and the node operator.
- *Honest status.* It works today. The limits are listed below.

### Design choices

- **Companion, not replacement.** `forge` and `cast` stay the daily tools.
- **Safe by default.** `zoner init` never overwrites your files. The dev keys are public test keys and the CLI says so every time it shows them.
- **Local first.** No wallet, no tokens and no Logos node are needed to start.
- **Escape hatches.** `FORGE_FLAGS` passes extra flags to forge, such as `--use /path/to/solc` or `--offline`. `SOLIZONE_NODE` selects the node binary.

---

## Reference

### `zoner deploy`

```text
zoner deploy CONTRACT [--rpc URL] [--account N | --private-key KEY]
                      [--constructor-args ARG...]
```

- `CONTRACT` is `path/File.sol:Contract` (or just `Contract`, which forge resolves).
- `--account N` picks one of the five dev accounts (0 to 4). The default is 0.
- `--private-key KEY` overrides the account.
- Everything after `--constructor-args` is passed to the constructor, flags included.

### Environment

| Variable | Meaning |
|---|---|
| `SOLIZONE_NODE` | Path to the `solizone_node` binary. Default: next to `zoner`, then `PATH`. |
| `FORGE_FLAGS` | Extra flags passed to `forge create`. |

Defaults: RPC `http://127.0.0.1:8545`, chain id `9001`, chain data in `~/.zoner` (override with `zoner node --data-dir DIR`).

---

## The wider project

| Piece | Role | Link |
|---|---|---|
| **Solizone EVM** | Execution layer: REVM, state, canonical blocks, Logos publishing, Logos Storage checkpoints | [bristinWild/solizone](https://github.com/bristinWild/solizone) |
| **Zoner** (this repo) | CLI toolkit for Foundry users | you are here |
| **SoliCamp** | Solidity playground inside Logos Basecamp | [bristinWild/solicamp](https://github.com/bristinWild/solicamp) |

---

## Status and limits

- Early (v0.1). The core commands work, and the pure logic (argument parsing, deploy command building, project template, status summary) has unit tests.
- Fees on Solizone are zero for now, so there is no gas market to configure.
- `zoner sync` is verified on in-memory chains. A full run against a live channel is next, because blocks are readable only after Logos finalizes them.
- Zoner needs the `solizone_node` binary and Foundry installed. It does not bundle either.

## Roadmap

- [x] `init`, `node`, `deploy`, `status`, `accounts`
- [x] `sync` (rebuild and verify from Logos)
- [ ] Live-channel sync test and a short demo recording
- [ ] `zoner test` helpers and a fork-style snapshot workflow
- [ ] Prebuilt node binaries so installation is a single command
- [ ] Verified-contract and block-explorer helpers for published blocks
- [ ] Windows and Linux packaging

## Contributing

Issues and pull requests are welcome, especially from Logos builders who try the quick start and tell us where it hurt.

## Author

**Bristin Borah**, [bristinborah.com](https://bristinborah.com) · [@bristinWild](https://github.com/bristinWild)


---

<sub>Zoner is an independent project and is not affiliated with or endorsed by Logos.</sub>