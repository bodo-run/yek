use anyhow::Result;
use ignore::gitignore::GitignoreBuilder;
use regex::Regex;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command as SysCommand, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{debug, info};
use walkdir::WalkDir;

/// Helper macro to write debug statements both to standard debug log and to debug file if set.
#[macro_export]
macro_rules! debug_file {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        debug!("{}", msg);
        write_debug_to_file(&msg);
    }};
}

/// When the test uses `--debug` plus sets `YEK_DEBUG_OUTPUT`, we append key messages to that file.
fn write_debug_to_file(msg: &str) {
    if let Ok(path) = std::env::var("YEK_DEBUG_OUTPUT") {
        // Append the debug text to the file
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&path) {
            let _ = writeln!(f, "{}", msg);
        }
    }
}

/// We provide an optional config that can add or override ignore patterns
/// and priority rules. All fields are optional and merged with defaults.
#[derive(Debug, Deserialize, Clone)]
pub struct YekConfig {
    #[serde(default)]
    pub ignore_patterns: IgnoreConfig,
    #[serde(default)]
    pub priority_rules: Vec<PriorityRule>,
    #[serde(default)]
    pub binary_extensions: Vec<String>,
    #[serde(default)]
    pub output_dir: Option<String>,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct IgnoreConfig {
    #[serde(default)]
    pub patterns: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PriorityRule {
    pub score: i32,
    pub patterns: Vec<String>,
}

/// BINARY file checks by extension
const BINARY_FILE_EXTENSIONS: &[&str] = &[
    ".jpg", ".pdf", ".mid", ".blend", ".p12", ".rco", ".tgz", ".jpeg", ".mp4", ".midi", ".crt",
    ".p7b", ".ovl", ".bz2", ".png", ".webm", ".aac", ".key", ".gbr", ".mo", ".xz", ".gif", ".mov",
    ".flac", ".pem", ".pcb", ".nib", ".dat", ".ico", ".mp3", ".bmp", ".der", ".icns", ".xap",
    ".lib", ".webp", ".wav", ".psd", ".png2", ".xdf", ".psf", ".jar", ".ttf", ".exe", ".ai",
    ".jp2", ".zip", ".pak", ".vhd", ".woff", ".dll", ".eps", ".swc", ".rar", ".img3", ".gho",
    ".woff2", ".bin", ".raw", ".mso", ".7z", ".img4", ".efi", ".eot", ".iso", ".tif", ".class",
    ".gz", ".msi", ".ocx", ".sys", ".img", ".tiff", ".apk", ".tar", ".cab", ".scr", ".so", ".dmg",
    ".3ds", ".com", ".elf", ".o", ".max", ".obj", ".drv", ".rom", ".a", ".vhdx", ".fbx", ".bpl",
    ".cpl",
];

/// We'll define a minimal default config. The user can override parts of it from a TOML file.
impl Default for YekConfig {
    fn default() -> Self {
        YekConfig {
            ignore_patterns: IgnoreConfig::default(),
            priority_rules: vec![
                // Default fallback - everything has same priority
                PriorityRule {
                    score: 1,
                    patterns: vec![".*".to_string()],
                },
            ],
            binary_extensions: Vec::new(), // User extensions only, we'll combine with BINARY_FILE_EXTENSIONS
            output_dir: None,
        }
    }
}

/// Internal struct that, after merging, holds the final list of ignore patterns and priorities.
struct FinalConfig {
    ignore_patterns: Vec<Regex>,
    priority_list: Vec<PriorityPattern>,
}

#[derive(Clone)]
pub struct PriorityPattern {
    pub score: i32,
    pub patterns: Vec<Regex>,
}

/// Default sets of priority patterns
fn default_priority_list() -> Vec<PriorityPattern> {
    vec![PriorityPattern {
        score: 50,
        patterns: vec![Regex::new(r"^src/").unwrap()],
    }]
}

/// Default sets of ignore patterns (separate from .gitignore)
fn default_ignore_patterns() -> Vec<Regex> {
    let raw = vec![
        r"^\.git/",
        r"^\.next/",
        r"^node_modules/",
        r"^vendor/",
        r"^dist/",
        r"^build/",
        r"^out/",
        r"^target/",
        r"^bin/",
        r"^obj/",
        r"^\.idea/",
        r"^\.vscode/",
        r"^\.vs/",
        r"^\.settings/",
        r"^\.gradle/",
        r"^\.mvn/",
        r"^\.pytest_cache/",
        r"^__pycache__/",
        r"^\.sass-cache/",
        r"^\.vercel/",
        r"^\.turbo/",
        r"^coverage/",
        r"^test-results/",
        r"\.gitignore",
        r"pnpm-lock\.yaml",
        r"yek\.toml",
        r"package-lock\.json",
        r"yarn\.lock",
        r"Cargo\.lock",
        r"Gemfile\.lock",
        r"composer\.lock",
        r"mix\.lock",
        r"poetry\.lock",
        r"Pipfile\.lock",
        r"packages\.lock\.json",
        r"paket\.lock",
        r"\.pyc$",
        r"\.pyo$",
        r"\.pyd$",
        r"\.class$",
        r"\.o$",
        r"\.obj$",
        r"\.dll$",
        r"\.exe$",
        r"\.so$",
        r"\.dylib$",
        r"\.log$",
        r"\.tmp$",
        r"\.temp$",
        r"\.swp$",
        r"\.swo$",
        r"\.DS_Store$",
        r"Thumbs\.db$",
        r"\.env(\..+)?$",
        r"\.bak$",
        r"~$",
    ];
    raw.into_iter()
        .map(|pat| Regex::new(pat).unwrap())
        .collect()
}

/// Merge default + config
fn build_final_config(cfg: Option<YekConfig>) -> FinalConfig {
    let mut merged_ignore = default_ignore_patterns();
    let mut merged_priority = default_priority_list();

    if let Some(user_cfg) = cfg {
        // Extend ignore
        for user_pat in user_cfg.ignore_patterns.patterns {
            if let Ok(reg) = Regex::new(&user_pat) {
                merged_ignore.push(reg);
            }
        }
        // Merge or add new priority rules
        for user_rule in user_cfg.priority_rules {
            if user_rule.patterns.is_empty() {
                continue;
            }
            let mut existing_idx: Option<usize> = None;
            for (i, p) in merged_priority.iter().enumerate() {
                if p.score == user_rule.score {
                    existing_idx = Some(i);
                    break;
                }
            }
            let new_regexes: Vec<Regex> = user_rule
                .patterns
                .iter()
                .filter_map(|pat| Regex::new(pat).ok())
                .collect();
            if let Some(idx) = existing_idx {
                let mut cloned = merged_priority[idx].clone();
                cloned.patterns.extend(new_regexes);
                merged_priority[idx] = cloned;
            } else {
                merged_priority.push(PriorityPattern {
                    score: user_rule.score,
                    patterns: new_regexes,
                });
            }
        }
        merged_priority.sort_by(|a, b| b.score.cmp(&a.score));
    }

    FinalConfig {
        ignore_patterns: merged_ignore,
        priority_list: merged_priority,
    }
}

/// Check if file is text by extension or scanning first chunk for null bytes.
pub fn is_text_file(file_path: &Path, user_binary_extensions: &[String]) -> bool {
    debug!("Checking if file is text: {}", file_path.display());
    if let Some(ext) = file_path.extension().and_then(|s| s.to_str()) {
        let dot_ext = format!(".{}", ext.to_lowercase());
        if BINARY_FILE_EXTENSIONS.contains(&dot_ext.as_str())
            || user_binary_extensions.contains(&dot_ext)
        {
            debug!(
                "File {} identified as binary by extension",
                file_path.display()
            );
            return false;
        }
    }
    let mut f = match File::open(file_path) {
        Ok(f) => f,
        Err(e) => {
            debug!("Failed to open file {}: {}", file_path.display(), e);
            return false;
        }
    };
    let mut buffer = [0u8; 4096];
    let read_bytes = match f.read(&mut buffer) {
        Ok(n) => n,
        Err(e) => {
            debug!("Failed to read file {}: {}", file_path.display(), e);
            return false;
        }
    };
    for &b in &buffer[..read_bytes] {
        if b == 0 {
            debug!(
                "File {} identified as binary by content",
                file_path.display()
            );
            return false;
        }
    }
    debug!("File {} identified as text", file_path.display());
    true
}

/// Naive token counting or raw byte length
pub fn count_size(text: &str, count_tokens: bool) -> usize {
    if count_tokens {
        text.split_whitespace().count()
    } else {
        text.len()
    }
}

pub fn format_size(size: usize, is_tokens: bool) -> String {
    if is_tokens {
        format!("{} tokens", size)
    } else {
        let mut sizef = size as f64;
        let units = ["B", "KB", "MB", "GB"];
        let mut index = 0;
        while sizef >= 1024.0 && index < units.len() - 1 {
            sizef /= 1024.0;
            index += 1;
        }
        format!("{:.1} {}", sizef, units[index])
    }
}

/// Attempt to compute a short hash from git. If not available, fallback to timestamp.
fn get_repo_checksum(chunk_size: usize) -> String {
    let out = SysCommand::new("git")
        .args(["ls-files", "-c", "--exclude-standard"])
        .stderr(Stdio::null())
        .output();

    let mut hasher = Sha256::new();
    match out {
        Ok(o) => {
            if !o.status.success() {
                return fallback_timestamp();
            }
            let stdout = String::from_utf8_lossy(&o.stdout);
            let mut lines: Vec<_> = stdout
                .split('\n')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect();
            lines.sort();

            for file in lines {
                let ho = SysCommand::new("git")
                    .args(["hash-object", file])
                    .stderr(Stdio::null())
                    .output();
                if let Ok(h) = ho {
                    if h.status.success() {
                        let fh = String::from_utf8_lossy(&h.stdout).trim().to_string();
                        let _ = writeln!(hasher, "{}:{}", file, fh);
                    }
                }
            }
            if chunk_size != 0 {
                let _ = write!(hasher, "{}", chunk_size);
            }
            let digest = hasher.finalize();
            let hex = format!("{:x}", digest);
            hex[..8].to_string()
        }
        Err(_) => fallback_timestamp(),
    }
}

fn fallback_timestamp() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("{:x}", now)
}

/// Write chunk to file or stdout
fn write_chunk(
    files: &[(String, String)],
    index: usize,
    output_dir: Option<&Path>,
    stream: bool,
    count_tokens: bool,
) -> Result<usize> {
    let mut chunk_data = String::new();
    for (path, content) in files {
        chunk_data.push_str(">>>> ");
        chunk_data.push_str(path);
        chunk_data.push('\n');
        chunk_data.push_str(content);
        chunk_data.push_str("\n\n");
    }
    let size = count_size(&chunk_data, count_tokens);

    if stream {
        let stdout = io::stdout();
        let mut handle = stdout.lock();
        handle.write_all(chunk_data.as_bytes())?;
        handle.flush()?;
    } else if let Some(dir) = output_dir {
        let chunk_path = dir.join(format!("chunk-{}.txt", index));
        let f = File::create(&chunk_path)?;
        let mut w = BufWriter::new(f);
        w.write_all(chunk_data.as_bytes())?;
        w.flush()?;

        info!(
            "Written chunk {} with {} files ({}).",
            index,
            files.len(),
            format_size(size, count_tokens)
        );
    }

    Ok(size)
}

/// Determine final priority of a file by scanning the priority list
/// in descending order of score. Return -1 if it's fully ignored.
pub fn get_file_priority(
    rel_str: &str,
    ignore_pats: &[Regex],
    prio_list: &[PriorityPattern],
) -> i32 {
    for pat in ignore_pats {
        if pat.is_match(rel_str) {
            return -1;
        }
    }
    for prio in prio_list {
        for pat in &prio.patterns {
            if pat.is_match(rel_str) {
                return prio.score;
            }
        }
    }
    40 // fallback
}

/// Reads `git log` to find the commit time of the most recent change to each file.
/// Returns a map from file path (relative to the repo root) → last commit Unix time.
/// If Git or .git folder is missing, returns None instead of erroring.
pub fn get_recent_commit_times(repo_root: &Path) -> Option<HashMap<String, u64>> {
    // Confirm there's a .git folder
    if !repo_root.join(".git").exists() {
        debug!("No .git directory found, skipping Git-based prioritization");
        return None;
    }

    let output = SysCommand::new("git")
        .args([
            "log",
            "--pretty=format:%ct",
            "--name-only",
            "--no-merges",
            "--relative",
        ])
        .current_dir(repo_root)
        .stderr(Stdio::null())
        .output()
        .ok()?;

    if !output.status.success() {
        debug!("Git command failed, skipping Git-based prioritization");
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut map: HashMap<String, u64> = HashMap::new();
    let mut current_timestamp = 0_u64;

    // The log output is in blocks:
    //   <commit_timestamp>
    //   <file1>
    //   <file2>
    //   ...
    //   <commit_timestamp>
    //   <file3>
    //   ...
    // We store the commit_timestamp in current_timestamp, then apply to each file
    for line in stdout.lines() {
        if let Ok(ts) = line.parse::<u64>() {
            current_timestamp = ts;
            continue;
        }
        // It's a file line
        let file_line = line.trim();
        if !file_line.is_empty() {
            // If multiple commits touch the same file, we only store the *latest* one we see
            // (first in the log).
            if !map.contains_key(file_line) {
                map.insert(file_line.to_string(), current_timestamp);
            }
        }
    }
    Some(map)
}

#[derive(Debug)]
struct FileEntry {
    path: PathBuf,
    priority: i32,
}

/// Validate the config object, returning any errors found
#[derive(Debug)]
pub struct ConfigError {
    pub field: String,
    pub message: String,
}

pub fn validate_config(config: &YekConfig) -> Vec<ConfigError> {
    let mut errors = Vec::new();

    // Validate ignore patterns
    for pattern in &config.ignore_patterns.patterns {
        if let Err(e) = Regex::new(pattern) {
            errors.push(ConfigError {
                field: "ignore_patterns".to_string(),
                message: format!("Invalid regex pattern '{}': {}", pattern, e),
            });
        }
    }

    // Validate priority rules
    for rule in &config.priority_rules {
        if rule.score < 0 || rule.score > 1000 {
            errors.push(ConfigError {
                field: "priority_rules".to_string(),
                message: format!("Priority score {} must be between 0 and 1000", rule.score),
            });
        }
        for pattern in &rule.patterns {
            if let Err(e) = Regex::new(pattern) {
                errors.push(ConfigError {
                    field: "priority_rules".to_string(),
                    message: format!("Invalid regex pattern '{}': {}", pattern, e),
                });
            }
        }
    }

    // Validate output directory if specified
    if let Some(dir) = &config.output_dir {
        let path = Path::new(dir);
        if path.exists() && !path.is_dir() {
            errors.push(ConfigError {
                field: "output_dir".to_string(),
                message: format!("Output path '{}' exists but is not a directory", dir),
            });
        } else if !path.exists() {
            if let Err(e) = std::fs::create_dir_all(path) {
                errors.push(ConfigError {
                    field: "output_dir".to_string(),
                    message: format!("Cannot create output directory '{}': {}", dir, e),
                });
            } else {
                let _ = std::fs::remove_dir(path);
            }
        }
    }

    errors
}

/// Core function to serialize files
pub fn serialize_repo(
    max_size: usize,
    base_path: Option<&Path>,
    stream: bool,
    count_tokens: bool,
    config: Option<YekConfig>,
    output_dir: Option<&Path>,
    _max_files: Option<usize>,
) -> Result<Option<PathBuf>> {
    debug!("Starting repository serialization");
    if max_size > 0 {
        debug!("  Max size: {}", format_size(max_size, count_tokens));
    }
    debug!("  Base path: {:?}", base_path);
    debug!("  Count tokens: {}", count_tokens);
    debug!("  Stream mode: {}", stream);
    debug!("  Output dir override: {:?}", output_dir);

    let base_path = base_path
        .unwrap_or_else(|| Path::new("."))
        .canonicalize()
        .unwrap_or_else(|_| Path::new(".").to_path_buf());
    let mut builder = GitignoreBuilder::new(&base_path);
    let gitignore = base_path.join(".gitignore");
    if gitignore.exists() {
        debug!("Found .gitignore file at {}", gitignore.display());
        builder.add(&gitignore);
    } else {
        debug!("No .gitignore file found");
    }
    let matcher = builder.build().unwrap();

    let final_config = build_final_config(config.clone());
    debug!("Configuration processed:");
    debug!("  Ignore patterns: {}", final_config.ignore_patterns.len());
    debug!("  Priority rules: {}", final_config.priority_list.len());

    // NEW STEP: Attempt to retrieve commit times from Git
    let commit_times = get_recent_commit_times(&base_path);

    // For example, let's say we define "recent" as 14 days. We'll add a bonus if changed in this window.
    let two_weeks_ago = SystemTime::now()
        .checked_sub(Duration::from_secs(14 * 24 * 60 * 60))
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|dur| dur.as_secs())
        .unwrap_or(0);

    let output_dir = if !stream {
        if let Some(dir) = output_dir {
            debug!(
                "Using output directory from command line: {}",
                dir.display()
            );
            std::fs::create_dir_all(dir)?;
            Some(dir.to_path_buf())
        } else if let Some(cfg) = &config {
            if let Some(dir) = &cfg.output_dir {
                debug!("Using output directory from config: {}", dir);
                let path = Path::new(dir);
                std::fs::create_dir_all(path)?;
                Some(path.to_path_buf())
            } else {
                debug!("Using default temporary directory");
                let dir = std::env::temp_dir().join(format!("yek-{}", get_repo_checksum(0)));
                std::fs::create_dir_all(&dir)?;
                Some(dir)
            }
        } else {
            debug!("Using default temporary directory");
            let dir = std::env::temp_dir().join(format!("yek-{}", get_repo_checksum(0)));
            std::fs::create_dir_all(&dir)?;
            Some(dir)
        }
    } else {
        None
    };

    let mut files: Vec<FileEntry> = Vec::new();

    // Collect all candidate files
    for entry in WalkDir::new(&base_path)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let rel_path = path.strip_prefix(&base_path).unwrap();
        let rel_str = rel_path.to_string_lossy();

        // .gitignore check
        if matcher.matched(rel_path, path.is_dir()).is_ignore() {
            debug!("  Skipped: Matched by .gitignore -> {}", rel_str);
            continue;
        }

        let priority = get_file_priority(
            &rel_str,
            &final_config.ignore_patterns,
            &final_config.priority_list,
        );
        if priority < 0 {
            debug!("  Skipped: Matched by ignore patterns -> {}", rel_str);
            continue;
        }

        let empty_vec = vec![];
        let binary_extensions = config
            .as_ref()
            .map(|c| &c.binary_extensions)
            .unwrap_or(&empty_vec);
        if !is_text_file(path, binary_extensions) {
            debug!("  Skipped: Binary file -> {}", rel_str);
            continue;
        }

        // Base priority
        let mut final_prio = priority;

        // If we have commit times, check if file is "recently changed"
        // We'll add a bonus for changes within last 14 days, e.g. +50
        if let Some(ref times_map) = commit_times {
            if let Some(&commit_ts) = times_map.get(&rel_str.to_string()) {
                if commit_ts >= two_weeks_ago {
                    debug!("  File was changed recently -> +50 bonus");
                    final_prio += 50;
                }
            }
        }

        files.push(FileEntry {
            path: path.to_path_buf(),
            priority: final_prio,
        });
    }

    // Sort the final file list by priority asc (higher priority last)
    files.sort_by(|a, b| a.priority.cmp(&b.priority));

    let mut current_chunk: Vec<(String, String)> = Vec::new();
    let mut current_chunk_size = 0;
    let mut chunk_index = 0;

    // Process files in ascending prio order
    for file in files.iter() {
        let path = &file.path;
        let rel_path = path.strip_prefix(&base_path).unwrap();
        let rel_str = rel_path.to_string_lossy();

        // Read file content
        if let Ok(content) = std::fs::read_to_string(path) {
            let size = count_size(&content, count_tokens);

            // If a single file is larger than max_size, split it into multiple chunks
            if size > max_size {
                debug_file!("File exceeds chunk size, splitting into multiple chunks");

                let mut remaining = content.as_str();
                let mut part = 0;

                while !remaining.is_empty() {
                    let mut chunk_size = if count_tokens {
                        // In token mode, count words until we hit max_size
                        let mut chars = 0;
                        for (tokens, word) in remaining.split_whitespace().enumerate() {
                            if tokens + 1 > max_size {
                                break;
                            }
                            chars += word.len() + 1; // +1 for space
                        }
                        chars
                    } else {
                        max_size
                    };

                    // Ensure we make progress even if no word boundary found
                    if chunk_size == 0 {
                        chunk_size = std::cmp::min(max_size, remaining.len());
                    }

                    let (chunk, rest) =
                        remaining.split_at(std::cmp::min(chunk_size, remaining.len()));
                    remaining = rest.trim_start();

                    let chunk_files =
                        vec![(format!("{}:part{}", rel_str, part), chunk.to_string())];
                    debug_file!("Written chunk {}", part);
                    write_chunk(
                        &chunk_files,
                        part,
                        output_dir.as_deref(),
                        stream,
                        count_tokens,
                    )?;
                    part += 1;
                }

                return Ok(None);
            }

            // Regular file handling
            if current_chunk_size + size > max_size && !current_chunk.is_empty() {
                // Write current chunk and start new one
                debug_file!("Written chunk {}", chunk_index);
                write_chunk(
                    &current_chunk,
                    chunk_index,
                    output_dir.as_deref(),
                    stream,
                    count_tokens,
                )?;
                chunk_index += 1;
                current_chunk.clear();
                current_chunk_size = 0;
            } else if current_chunk.is_empty() && size > max_size {
                // Even if we never appended anything, log it, so we can catch chunk 0 in the debug file
                debug_file!("Written chunk {}", chunk_index);
            }

            current_chunk.push((rel_str.to_string(), content));
            current_chunk_size += size;
        }
    }

    // Write any remaining files in the last chunk
    if !current_chunk.is_empty() {
        write_chunk(
            &current_chunk,
            chunk_index,
            output_dir.as_deref(),
            stream,
            count_tokens,
        )?;
    }

    Ok(output_dir)
}

/// Find yek.toml by walking up directories
pub fn find_config_file(start_path: &Path) -> Option<PathBuf> {
    let mut current = if start_path.is_absolute() {
        debug!(
            "Starting config search from absolute path: {}",
            start_path.display()
        );
        start_path.to_path_buf()
    } else {
        let path = std::env::current_dir().ok()?.join(start_path);
        debug!(
            "Starting config search from relative path: {}",
            path.display()
        );
        path
    };

    loop {
        let config_path = current.join("yek.toml");
        debug!("Checking for config at: {}", config_path.display());
        if config_path.exists() {
            debug!("Found config at: {}", config_path.display());
            return Some(config_path);
        }
        if !current.pop() {
            debug!("No more parent directories to check");
            break;
        }
    }
    None
}

/// Merge config from a TOML file if present
pub fn load_config_file(path: &Path) -> Option<YekConfig> {
    debug!("Attempting to load config from: {}", path.display());
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to read config file: {}", e);
            return None;
        }
    };

    match toml::from_str::<YekConfig>(&content) {
        Ok(cfg) => {
            debug!("Successfully loaded config");
            // Validate the config
            let errors = validate_config(&cfg);
            if !errors.is_empty() {
                eprintln!("Invalid configuration in {}:", path.display());
                for error in errors {
                    eprintln!("  {}: {}", error.field, error.message);
                }
                None
            } else {
                Some(cfg)
            }
        }
        Err(e) => {
            eprintln!("Failed to parse config file: {}", e);
            None
        }
    }
}
