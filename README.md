# Zoner

Developer tooling for [Solizone EVM](https://github.com/bristinWild/solizone),
a thin companion to [Foundry](https://getfoundry.sh).

Zoner talks to a Solizone node only through its JSON-RPC endpoint and the
`solizone_node` binary. It has no code dependency on the node crate.

## Install

    cargo install --path .

Needs Foundry (`forge`) on PATH for `init`/`deploy`.

## Commands

    zoner init [DIR]            new Foundry project preconfigured for Solizone (chain 9001)
    zoner node [flags]          run solizone_node (defaults to --storage local)
    zoner deploy CONTRACT       forge create against the node
        [--rpc URL] [--account N | --private-key K] [--constructor-args ...]
    zoner status [--rpc URL]    chain id, head, publication status
    zoner accounts              the dev accounts

## Environment

- `SOLIZONE_NODE`: path to the `solizone_node` binary (default: next to the zoner binary, else PATH)
- `FORGE_FLAGS`: extra flags passed to forge (e.g. `--use /path/to/solc`)
- default RPC: `http://127.0.0.1:8545`