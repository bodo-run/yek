use anyhow::Result;
use byte_unit::Byte;
use clap::{Arg, ArgAction, Command};
use std::io::IsTerminal;
use std::path::Path;
use tracing::{info, Level};
use tracing_subscriber::fmt;
use yek::{find_config_file, load_config_file, serialize_repo};

fn parse_size_input(input: &str) -> std::result::Result<usize, String> {
    Byte::from_str(input)
        .map(|b| b.get_bytes() as usize)
        .map_err(|e| e.to_string())
}

fn main() -> Result<()> {
    let matches = Command::new("yek")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Repository content chunker and serializer for LLM consumption")
        .arg(
            Arg::new("max-size")
                .long("max-size")
                .help("Maximum size per chunk (e.g. '10MB', '128KB', '1GB')")
                .default_value("10MB"),
        )
        .arg(
            Arg::new("tokens")
                .long("tokens")
                .help("Count size in tokens instead of bytes")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("debug")
                .long("debug")
                .help("Enable debug output")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("output-dir")
                .long("output-dir")
                .help("Output directory for chunks"),
        )
        .get_matches();

    // Setup logging
    let level = if matches.get_flag("debug") {
        Level::DEBUG
    } else {
        Level::INFO
    };
    fmt()
        .with_max_level(level)
        .with_target(false)
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_ansi(true)
        .init();

    // Parse max size
    let max_size_str = matches.get_one::<String>("max-size").unwrap();
    let max_size = parse_size_input(max_size_str).map_err(|e| anyhow::anyhow!(e))?;

    // Get current directory
    let current_dir = std::env::current_dir()?;

    // Find config file
    let config = find_config_file(&current_dir).and_then(|p| load_config_file(&p));

    // Get output directory from command line or config
    let output_dir = matches
        .get_one::<String>("output-dir")
        .map(|s| Path::new(s).to_path_buf());

    // Check if we're in stream mode (piped output)
    let stream = output_dir.is_none() && !std::io::stdout().is_terminal();

    if let Some(output_path) = serialize_repo(
        max_size,
        Some(&current_dir),
        stream,
        matches.get_flag("tokens"),
        config,
        output_dir.as_deref(),
        None,
    )? {
        info!("Output written to {}", output_path.display());
    }

    Ok(())
}
