//! Zoner: a thin companion to Foundry for Solizone.
//!
//! ```text
//! zoner init [DIR]
//! zoner node [solizone_node flags...]
//! zoner deploy CONTRACT [--rpc URL] [--account N | --private-key KEY] [--constructor-args ARG...]
//! zoner status [--rpc URL]
//! zoner accounts
//! ```
//!
//! Foundry (`forge`, `cast`) stays the toolchain; see the `zoner` library.

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command as Process, ExitCode},
};

use jsonrpsee::{core::client::ClientT, http_client::HttpClientBuilder, rpc_params};
use serde_json::Value;

use zoner::{Command, DEV_ACCOUNT_BALANCE_WEI, DEV_ACCOUNTS};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();

    let result = zoner::parse(&args).and_then(run);

    match result {
        Ok(code) => code,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn run(command: Command) -> Result<ExitCode, String> {
    match command {
        Command::Help => {
            println!("{}", zoner::usage());
            Ok(ExitCode::SUCCESS)
        }
        Command::Accounts => {
            accounts();
            Ok(ExitCode::SUCCESS)
        }
        Command::Init { dir } => {
            init(Path::new(&dir))?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Node { args } => node(args),
        Command::Deploy(options) => deploy(&options),
        Command::Status { rpc } => {
            status(&rpc)?;
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn accounts() {
    println!("Pre-funded dev accounts (PUBLIC test keys, never use for real value):");
    println!("balance each: {DEV_ACCOUNT_BALANCE_WEI} wei");

    for (index, account) in DEV_ACCOUNTS.iter().enumerate() {
        println!("  ({index}) {}  {}", account.address, account.private_key);
    }
}

fn init(dir: &Path) -> Result<(), String> {
    let files = zoner::init_files();

    // Never overwrite anything the user already has.
    for (name, _) in &files {
        let path = dir.join(name);

        if path.exists() {
            return Err(format!(
                "{} already exists; zoner init does not overwrite files",
                path.display()
            ));
        }
    }

    for (name, contents) in &files {
        let path = dir.join(name);

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }

        fs::write(&path, contents)
            .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
    }

    println!("Created a Solizone project in {}", dir.display());
    println!("  zoner node                            start a local node");
    println!("  zoner deploy src/Counter.sol:Counter  deploy the example contract");

    Ok(())
}

/// The node binary: `$SOLIZONE_NODE` if set, else `solizone_node` next to this
/// executable, else whatever is on `PATH`.
fn node_binary() -> PathBuf {
    if let Some(path) = env::var_os("SOLIZONE_NODE") {
        return PathBuf::from(path);
    }

    let name = format!("solizone_node{}", env::consts::EXE_SUFFIX);

    env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(&name)))
        .filter(|path| path.exists())
        .unwrap_or_else(|| PathBuf::from(name))
}

fn node(mut args: Vec<String>) -> Result<ExitCode, String> {
    // Flags go after a leading subcommand such as `reset`, if there is one.
    let at = usize::from(args.first().is_some_and(|arg| !arg.starts_with('-')));

    // Local development defaults to local storage: no Basecamp needed.
    if !args.iter().any(|arg| arg == "--storage") {
        args.splice(at..at, ["--storage".to_string(), "local".to_string()]);
    }

    // Keep chain data out of whatever folder the command runs in.
    if !args.iter().any(|arg| arg == "--data-dir")
        && let Some(dir) = default_data_dir()
    {
        fs::create_dir_all(&dir)
            .map_err(|error| format!("failed to create {}: {error}", dir.display()))?;

        args.splice(
            at..at,
            ["--data-dir".to_string(), dir.to_string_lossy().into_owned()],
        );
    }

    let binary = node_binary();

    let status = Process::new(&binary)
        .args(&args)
        .status()
        .map_err(|error| {
            format!(
                "failed to start {} ({error}); set SOLIZONE_NODE to the solizone_node \
             binary or put it on PATH",
                binary.display()
            )
        })?;

    Ok(exit_code(status))
}

/// `~/.zoner`, where `zoner node` keeps chain data unless told otherwise.
fn default_data_dir() -> Option<PathBuf> {
    env::var_os("HOME").map(|home| PathBuf::from(home).join(".zoner"))
}

fn deploy(options: &zoner::DeployOptions) -> Result<ExitCode, String> {
    let mut args = zoner::forge_create_args(options);

    // Extra forge flags, like the repository scripts take (e.g. "--offline").
    // They go before `--constructor-args`, which must stay last.
    if let Ok(extra) = env::var("FORGE_FLAGS") {
        let position = args
            .iter()
            .position(|arg| arg == "--constructor-args")
            .unwrap_or(args.len());

        args.splice(
            position..position,
            extra.split_whitespace().map(String::from),
        );
    }

    let status = Process::new("forge").args(args).status().map_err(|error| {
        format!("failed to run forge ({error}); install Foundry from https://getfoundry.sh")
    })?;

    Ok(exit_code(status))
}

fn exit_code(status: std::process::ExitStatus) -> ExitCode {
    if status.success() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn status(rpc: &str) -> Result<(), String> {
    let runtime = tokio::runtime::Runtime::new().map_err(|error| error.to_string())?;

    runtime.block_on(async {
        let client = HttpClientBuilder::default()
            .build(rpc)
            .map_err(|error| format!("invalid RPC url {rpc}: {error}"))?;

        let unreachable = |error: jsonrpsee::core::ClientError| {
            format!("cannot reach a Solizone node at {rpc}: {error}")
        };

        let chain_id: String = client
            .request("eth_chainId", rpc_params![])
            .await
            .map_err(unreachable)?;
        let block_number: String = client
            .request("eth_blockNumber", rpc_params![])
            .await
            .map_err(|error| error.to_string())?;
        let publication: Value = client
            .request("solizone_publicationStatus", rpc_params![])
            .await
            .map_err(|error| error.to_string())?;

        println!(
            "{}",
            zoner::status_summary(&chain_id, &block_number, &publication)
        );

        Ok(())
    })
}
