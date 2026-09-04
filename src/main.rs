use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use electronics_manufacturing_mcp::{config::AppConfig, mcp};
use serde_json::json;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(name = "electronics-manufacturing-mcp", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// 通过标准输入输出启动 MCP 服务。
    Serve {
        #[arg(long)]
        config: Option<PathBuf>,
        #[arg(long = "allow-root")]
        allowed_roots: Vec<PathBuf>,
    },
    /// 检查配置、文件访问范围和可选 KiCad CLI。
    Doctor {
        #[arg(long)]
        config: Option<PathBuf>,
        #[arg(long = "allow-root")]
        allowed_roots: Vec<PathBuf>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()))
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    match Cli::parse().command.unwrap_or(Command::Serve {
        config: None,
        allowed_roots: Vec::new(),
    }) {
        Command::Serve {
            config,
            allowed_roots,
        } => {
            let config = AppConfig::load(config.as_deref(), &allowed_roots)?;
            mcp::serve(config).await
        }
        Command::Doctor {
            config,
            allowed_roots,
        } => {
            let config = AppConfig::load(config.as_deref(), &allowed_roots)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "status": "ok",
                    "version": env!("CARGO_PKG_VERSION"),
                    "allowed_roots": config.filesystem.allowed_roots,
                    "kicad_cli": config.find_kicad_cli(),
                    "transport": "stdio"
                }))?
            );
            Ok(())
        }
    }
}
