//! Zoner: a thin companion to Foundry for Solizone.
//!
//! Foundry stays the toolchain for compiling, testing, deploying and calling
//! contracts. Zoner only adds what is specific to Solizone: project defaults
//! (chain id, RPC endpoint, dev accounts), starting the node with the right
//! flags, deploying with those defaults, and showing what has been published
//! to Logos.
//!
//! Zoner talks to Solizone only through its public interface: the Ethereum
//! JSON-RPC (plus `solizone_publicationStatus`) and the `solizone_node`
//! command line. It does not link the node's Rust code, so the two
//! repositories release independently.
//!
//! This module holds the logic that does no I/O (argument parsing, the files
//! `init` writes, the `forge` command lines, the status summary) so it can be
//! tested. The `zoner` binary does the I/O.

use serde_json::Value;

/// The pre-funded dev accounts of a Solizone dev chain: the well-known
/// Foundry/Anvil test accounts from the public mnemonic "test test ... junk".
/// Their private keys are PUBLIC; never use them for anything of value.
pub struct DevAccount {
    pub address: &'static str,
    pub private_key: &'static str,
}

pub const DEV_ACCOUNTS: [DevAccount; 5] = [
    DevAccount {
        address: "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
        private_key: "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
    },
    DevAccount {
        address: "0x70997970C51812dc3A010C7d01b50e0d17dc79C8",
        private_key: "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d",
    },
    DevAccount {
        address: "0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC",
        private_key: "0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a",
    },
    DevAccount {
        address: "0x90F79bf6EB2c4f870365E785982E1f101E93b906",
        private_key: "0x7c852118294e51e653712a81e05800f419141751be58f605c371e15141b007a6",
    },
    DevAccount {
        address: "0x15d34AAf54267DB7D7c367839AAf71A00a2C6A65",
        private_key: "0x47e179ec197488593b187f80a00eb0da91f1b9d0b13f8733639f19c30a34926a",
    },
];

/// Balance of each dev account on a fresh chain, in wei (10,000 ETH).
pub const DEV_ACCOUNT_BALANCE_WEI: &str = "10000000000000000000000";

pub const DEFAULT_RPC: &str = "http://127.0.0.1:8545";
pub const CHAIN_ID: u64 = 9001;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Create a Foundry project preconfigured for Solizone.
    Init {
        dir: String,
    },
    /// Start `solizone_node` (arguments after `node` are passed through).
    Node {
        args: Vec<String>,
    },
    /// Deploy a contract with `forge create`.
    Deploy(DeployOptions),
    /// Show node and Logos publication status.
    Status {
        rpc: String,
    },
    /// List the pre-funded dev accounts.
    Accounts,
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeployOptions {
    /// `path/File.sol:Contract` (or just `Contract`, which forge resolves).
    pub contract: String,
    pub rpc: String,
    /// Dev account index whose (public) key signs the deployment.
    pub account: usize,
    /// Explicit private key; overrides `account`.
    pub private_key: Option<String>,
    /// Constructor arguments, passed to forge after `--constructor-args`.
    pub constructor_args: Vec<String>,
}

pub fn usage() -> String {
    "\
zoner: Solizone companion for Foundry

usage:
  zoner init [DIR]                      create a Foundry project configured for Solizone
  zoner node [solizone_node flags...]   start a local node (default: --storage local)
  zoner sync [solizone_node flags...]   rebuild the chain from Logos (empty data dir),
                                        verifying every block, then run the node
  zoner deploy CONTRACT [--rpc URL] [--account N | --private-key KEY]
                        [--constructor-args ARG...]
  zoner status [--rpc URL]              block height and Logos publication status
  zoner accounts                        pre-funded dev accounts (PUBLIC test keys)

Use forge and cast for everything else (build, test, call, send)."
        .to_string()
}

pub fn parse(args: &[String]) -> Result<Command, String> {
    let Some((command, rest)) = args.split_first() else {
        return Ok(Command::Help);
    };

    match command.as_str() {
        "init" => match rest {
            [] => Ok(Command::Init { dir: ".".into() }),
            [dir] => Ok(Command::Init { dir: dir.clone() }),
            _ => Err(format!("init takes at most one directory\n{}", usage())),
        },
        "node" => Ok(Command::Node {
            args: rest.to_vec(),
        }),
        // Rebuild the chain from Logos, then run the node on top of it.
        "sync" => {
            let mut args = rest.to_vec();

            if !args.iter().any(|arg| arg == "--sync-from-logos") {
                args.push("--sync-from-logos".to_string());
            }

            Ok(Command::Node { args })
        }
        "deploy" => parse_deploy(rest).map(Command::Deploy),
        "status" => {
            let mut rpc = DEFAULT_RPC.to_string();
            let mut iter = rest.iter();

            while let Some(arg) = iter.next() {
                match arg.as_str() {
                    "--rpc" => rpc = next_value(&mut iter, "--rpc")?,
                    other => return Err(format!("unknown argument {other:?}\n{}", usage())),
                }
            }

            Ok(Command::Status { rpc })
        }
        "accounts" => Ok(Command::Accounts),
        "-h" | "--help" | "help" => Ok(Command::Help),
        other => Err(format!("unknown command {other:?}\n{}", usage())),
    }
}

fn next_value(iter: &mut std::slice::Iter<'_, String>, name: &str) -> Result<String, String> {
    iter.next()
        .cloned()
        .ok_or_else(|| format!("{name} needs a value\n{}", usage()))
}

fn parse_deploy(args: &[String]) -> Result<DeployOptions, String> {
    let mut contract = None;
    let mut options = DeployOptions {
        contract: String::new(),
        rpc: DEFAULT_RPC.to_string(),
        account: 0,
        private_key: None,
        constructor_args: Vec::new(),
    };

    let mut iter = args.iter();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--rpc" => options.rpc = next_value(&mut iter, "--rpc")?,
            "--account" => {
                options.account = next_value(&mut iter, "--account")?
                    .parse()
                    .map_err(|error| format!("invalid --account: {error}"))?;

                if options.account >= DEV_ACCOUNTS.len() {
                    return Err(format!("--account must be 0..{}", DEV_ACCOUNTS.len() - 1));
                }
            }
            "--private-key" => options.private_key = Some(next_value(&mut iter, "--private-key")?),
            // Everything after this is a constructor argument, flags included.
            "--constructor-args" => {
                options.constructor_args = iter.by_ref().cloned().collect();
            }
            other if other.starts_with("--") => {
                return Err(format!("unknown argument {other:?}\n{}", usage()));
            }
            other => {
                if contract.replace(other.to_string()).is_some() {
                    return Err(format!("only one contract can be deployed\n{}", usage()));
                }
            }
        }
    }

    options.contract = contract.ok_or_else(|| format!("deploy needs a contract\n{}", usage()))?;

    Ok(options)
}

/// The `forge create` arguments for a deployment. The constructor arguments
/// go last because forge treats everything after `--constructor-args` as one
/// list, flags included.
pub fn forge_create_args(options: &DeployOptions) -> Vec<String> {
    let key = options
        .private_key
        .clone()
        .unwrap_or_else(|| DEV_ACCOUNTS[options.account].private_key.to_string());

    let mut args = vec![
        "create".to_string(),
        options.contract.clone(),
        "--broadcast".to_string(),
        "--rpc-url".to_string(),
        options.rpc.clone(),
        "--private-key".to_string(),
        key,
    ];

    if !options.constructor_args.is_empty() {
        args.push("--constructor-args".to_string());
        args.extend(options.constructor_args.iter().cloned());
    }

    args
}

/// Files `zoner init` creates, relative to the project directory.
pub fn init_files() -> Vec<(&'static str, String)> {
    vec![
        (
            "foundry.toml",
            format!(
                "\
[profile.default]
src = \"src\"
out = \"out\"
libs = [\"lib\"]
solc_version = \"0.8.20\"

# Solizone local node (`zoner node`).
[rpc_endpoints]
solizone = \"{DEFAULT_RPC}\"
"
            ),
        ),
        (
            "src/Counter.sol",
            "\
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

contract Counter {
    uint256 public count;

    event Incremented(uint256 count);

    function increment() external {
        count += 1;
        emit Incremented(count);
    }
}
"
            .to_string(),
        ),
        (".gitignore", "out/\ncache/\n.env\n".to_string()),
        (
            "README.md",
            format!(
                "\
# Solizone project

```bash
zoner node                                  # start a local Solizone node (chain id {CHAIN_ID})
zoner deploy src/Counter.sol:Counter        # deploy with a pre-funded dev account
cast send <address> \"increment()\" --rpc-url {DEFAULT_RPC} --private-key <dev key>
zoner status                                # block height and Logos publication
```

`zoner accounts` lists the pre-funded dev accounts. Their private keys are
PUBLIC test keys: never use them for anything of value.
"
            ),
        ),
    ]
}

/// Human-readable status from `eth_blockNumber` / `eth_chainId` (hex strings)
/// and the `solizone_publicationStatus` result.
pub fn status_summary(chain_id: &str, block_number: &str, publication: &Value) -> String {
    let mut lines = Vec::new();

    lines.push(format!(
        "chain id:    {}",
        parse_hex(chain_id).map_or_else(|| chain_id.to_string(), |value| value.to_string())
    ));
    lines.push(format!(
        "head block:  #{}",
        parse_hex(block_number).map_or_else(|| block_number.to_string(), |value| value.to_string())
    ));

    if publication["enabled"].as_bool() != Some(true) {
        lines.push("publishing:  off (start the node with --publish)".to_string());
        return lines.join("\n");
    }

    let count = |field: &str| publication[field].as_u64().unwrap_or(0);

    lines.push(format!(
        "publishing:  next block #{}, {} not yet submitted, {} waiting for Logos finalization",
        count("nextHeight"),
        count("unsubmitted"),
        count("inFlight")
    ));

    if let Some(reason) = publication["failed"].as_str() {
        lines.push(format!("FAILED:      {reason}"));
    }

    if let Some(recent) = publication["recent"].as_array() {
        for entry in recent.iter().take(5) {
            lines.push(format!(
                "  block #{:<6} {}",
                entry["height"].as_u64().unwrap_or(0),
                entry["status"].as_str().unwrap_or("?")
            ));
        }
    }

    lines.join("\n")
}

fn parse_hex(value: &str) -> Option<u64> {
    u64::from_str_radix(value.trim_start_matches("0x"), 16).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn args(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| item.to_string()).collect()
    }

    #[test]
    fn no_arguments_shows_help() {
        assert_eq!(parse(&[]).unwrap(), Command::Help);
        assert_eq!(parse(&args(&["--help"])).unwrap(), Command::Help);
    }

    #[test]
    fn parses_init() {
        assert_eq!(
            parse(&args(&["init"])).unwrap(),
            Command::Init { dir: ".".into() }
        );
        assert_eq!(
            parse(&args(&["init", "my-zone"])).unwrap(),
            Command::Init {
                dir: "my-zone".into()
            }
        );
        assert!(parse(&args(&["init", "a", "b"])).is_err());
    }

    #[test]
    fn node_passes_flags_through() {
        assert_eq!(
            parse(&args(&["node", "--publish", "--block-time", "1"])).unwrap(),
            Command::Node {
                args: args(&["--publish", "--block-time", "1"])
            }
        );
    }

    #[test]
    fn sync_is_node_with_sync_from_logos() {
        assert_eq!(
            parse(&args(&["sync", "--logos-node", "http://x:8080"])).unwrap(),
            Command::Node {
                args: args(&["--logos-node", "http://x:8080", "--sync-from-logos"])
            }
        );
        // Not added twice.
        assert_eq!(
            parse(&args(&["sync", "--sync-from-logos"])).unwrap(),
            Command::Node {
                args: args(&["--sync-from-logos"])
            }
        );
    }

    #[test]
    fn parses_deploy_with_defaults_and_options() {
        let Command::Deploy(options) = parse(&args(&["deploy", "src/A.sol:A"])).unwrap() else {
            panic!("expected deploy");
        };

        assert_eq!(options.contract, "src/A.sol:A");
        assert_eq!(options.rpc, DEFAULT_RPC);
        assert_eq!(options.account, 0);
        assert!(options.constructor_args.is_empty());

        let Command::Deploy(options) = parse(&args(&[
            "deploy",
            "src/A.sol:A",
            "--rpc",
            "http://example:1",
            "--account",
            "2",
            "--constructor-args",
            "42",
            "--flag-looking",
        ]))
        .unwrap() else {
            panic!("expected deploy");
        };

        assert_eq!(options.rpc, "http://example:1");
        assert_eq!(options.account, 2);
        // Everything after --constructor-args belongs to the constructor.
        assert_eq!(options.constructor_args, args(&["42", "--flag-looking"]));
    }

    #[test]
    fn rejects_bad_deploy_input() {
        assert!(parse(&args(&["deploy"])).is_err());
        assert!(parse(&args(&["deploy", "A", "B"])).is_err());
        assert!(parse(&args(&["deploy", "A", "--account", "9"])).is_err());
        assert!(parse(&args(&["deploy", "A", "--account", "x"])).is_err());
        assert!(parse(&args(&["deploy", "A", "--nope"])).is_err());
        assert!(parse(&args(&["deploy", "A", "--rpc"])).is_err());
        assert!(parse(&args(&["frobnicate"])).is_err());
    }

    #[test]
    fn forge_args_use_the_dev_account_and_put_constructor_args_last() {
        let options = DeployOptions {
            contract: "src/A.sol:A".into(),
            rpc: DEFAULT_RPC.into(),
            account: 1,
            private_key: None,
            constructor_args: args(&["7"]),
        };

        let forge = forge_create_args(&options);

        assert_eq!(forge[0], "create");
        assert_eq!(forge[1], "src/A.sol:A");
        assert!(forge.contains(&"--broadcast".to_string()));
        assert!(forge.contains(&DEV_ACCOUNTS[1].private_key.to_string()));
        assert_eq!(
            &forge[forge.len() - 2..],
            &args(&["--constructor-args", "7"])[..]
        );
    }

    #[test]
    fn an_explicit_key_overrides_the_account() {
        let options = DeployOptions {
            contract: "A".into(),
            rpc: DEFAULT_RPC.into(),
            account: 0,
            private_key: Some("0xabc".into()),
            constructor_args: vec![],
        };

        let forge = forge_create_args(&options);

        assert!(forge.contains(&"0xabc".to_string()));
        assert!(!forge.contains(&DEV_ACCOUNTS[0].private_key.to_string()));
        assert!(!forge.contains(&"--constructor-args".to_string()));
    }

    #[test]
    fn init_files_describe_a_solizone_project() {
        let files = init_files();
        let names: Vec<&str> = files.iter().map(|(name, _)| *name).collect();

        assert_eq!(
            names,
            ["foundry.toml", "src/Counter.sol", ".gitignore", "README.md"]
        );

        let foundry = &files[0].1;
        assert!(foundry.contains("solc_version = \"0.8.20\""));
        assert!(foundry.contains(DEFAULT_RPC));
        assert!(files[1].1.contains("contract Counter"));
    }

    #[test]
    fn status_when_publishing_is_off() {
        let text = status_summary("0x2329", "0x5", &json!({"enabled": false}));

        assert!(text.contains("chain id:    9001"));
        assert!(text.contains("head block:  #5"));
        assert!(text.contains("publishing:  off"));
    }

    #[test]
    fn status_shows_publication_progress_and_failures() {
        let text = status_summary(
            "0x2329",
            "0x3",
            &json!({
                "enabled": true,
                "nextHeight": 4,
                "unsubmitted": 0,
                "inFlight": 4,
                "failed": "block too large",
                "recent": [
                    {"height": 3, "status": "submitted"},
                    {"height": 2, "status": "finalized"}
                ]
            }),
        );

        assert!(text.contains("next block #4"));
        assert!(text.contains("4 waiting for Logos finalization"));
        assert!(text.contains("FAILED:      block too large"));
        assert!(text.contains("block #3"));
        assert!(text.contains("finalized"));
    }
}
