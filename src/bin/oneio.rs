use clap::{Parser, Subcommand};
use std::io::{BufRead, BufReader, IsTerminal, Read, Write};
use std::path::PathBuf;
use std::process::exit;
use std::time::Duration;

/// Parse a header string. Accepts "Name: Value" or "Name:Value" (curl-compatible).
fn parse_header(s: &str) -> Result<(String, String), String> {
    let (name, value) = s
        .split_once(':')
        .ok_or_else(|| format!("invalid header format, expected 'Name: Value': {s}"))?;
    let name = name.trim().to_string();
    let value = value.trim().to_string();
    if name.is_empty() {
        return Err("header name cannot be empty".to_string());
    }
    Ok((name, value))
}

#[derive(Parser)]
#[clap(author, version)]
#[clap(propagate_version = true)]
#[command(arg_required_else_help(true))]
/// oneio reads files from local or remote locations with any compression.
struct Cli {
    /// File to open, remote or local
    #[clap(name = "FILE")]
    file: Option<PathBuf>,

    /// Download the file to the current directory (similar to wget)
    #[clap(short, long)]
    download: bool,

    /// Output file path
    #[clap(short, long)]
    outfile: Option<PathBuf>,

    /// Cache reading to a specified directory
    #[clap(long)]
    cache_dir: Option<String>,

    /// Force re-caching if a local cache already exists
    #[clap(long)]
    cache_force: bool,

    /// Specify cache file name
    #[clap(long)]
    cache_file: Option<String>,

    /// Read through the file and only print out stats
    #[clap(short, long)]
    stats: bool,

    /// Add HTTP header in "Name: Value" format, can be repeated (e.g. -H "Authorization: Bearer TOKEN")
    #[clap(short = 'H', long = "header", value_parser = clap::builder::ValueParser::new(parse_header))]
    headers: Vec<(String, String)>,

    /// Override compression type (gz, bz2, lz4, xz, zst). Ignored when --download is used.
    #[clap(long)]
    compression: Option<String>,

    #[clap(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// S3-related subcommands
    S3 {
        #[clap(subcommand)]
        s3_command: S3Commands,
    },

    /// Generate SHA256 digest
    Digest {
        /// File to open, remote or local
        #[clap(name = "FILE")]
        file: PathBuf,
    },
}

#[derive(Subcommand)]
enum S3Commands {
    /// Upload a local file to S3
    Upload {
        /// Local file to upload
        #[clap(name = "LOCAL_FILE")]
        local_file: PathBuf,

        /// S3 bucket name
        bucket: String,

        /// S3 key path
        path: String,
    },

    /// Download a file from S3
    Download {
        /// S3 bucket name
        bucket: String,

        /// S3 key path
        path: String,

        /// Local output file path (defaults to the filename from the S3 key)
        #[clap(short, long)]
        outfile: Option<PathBuf>,
    },

    /// List objects in an S3 bucket
    List {
        /// S3 bucket name
        bucket: String,

        /// Key prefix to filter results
        #[clap(default_value = "")]
        prefix: String,

        /// Delimiter for directory-style listing
        #[clap(short, long)]
        delimiter: Option<String>,

        /// Show directories only
        #[clap(short, long)]
        dirs: bool,
    },
}

/// Downloads `path` to `out_path` with a progress bar on stderr.
///
/// `indicatif::ProgressBar` is `Clone + Send + Sync`, so no Arc needed.
/// The bar is shown immediately; if the total size is unknown it shows a spinner.
fn download_with_progress(
    oneio: &oneio::OneIo,
    path: &str,
    out_path: &str,
    message: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let pb = indicatif::ProgressBar::new(0);
    pb.set_draw_target(indicatif::ProgressDrawTarget::stderr());
    pb.set_style(
        indicatif::ProgressStyle::default_bar()
            .template(
                "{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] \
                 {bytes}/{total_bytes} ({bytes_per_sec}, {eta})",
            )?
            .progress_chars("#>-"),
    );
    pb.set_message(message.to_string());
    pb.enable_steady_tick(Duration::from_millis(100));

    let pb_cb = pb.clone();
    let (mut reader, total_size) =
        oneio.get_reader_with_progress(path, move |bytes_read, total_bytes| {
            if total_bytes > 0 {
                pb_cb.set_length(total_bytes);
            }
            pb_cb.set_position(bytes_read);
        })?;

    // Set length upfront if we got it from the content-length probe.
    if let Some(size) = total_size {
        pb.set_length(size);
    }

    let mut writer = std::fs::File::create(out_path)?;
    let mut buffer = [0u8; 8192];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => writer.write_all(&buffer[..n])?,
            Err(e) => return Err(Box::new(e)),
        }
    }
    pb.finish_with_message(format!("Downloaded to {out_path}"));
    Ok(())
}

fn build_oneio(headers: &[(String, String)]) -> oneio::OneIo {
    let mut builder = oneio::OneIo::builder();
    for (name, value) in headers {
        builder = builder.header_str(name, value);
    }
    builder.build().unwrap_or_else(|e| {
        eprintln!("error: failed to create OneIo client: {e}");
        exit(1);
    })
}

fn s3_credentials_or_exit() {
    if let Err(e) = oneio::s3_env_check() {
        eprintln!("missing S3 credentials: {e}");
        exit(1);
    }
}

fn main() {
    let cli = Cli::parse();
    let outfile = cli.outfile;
    let use_progress = std::io::stderr().is_terminal();

    let oneio = build_oneio(&cli.headers);

    if let Some(command) = cli.command {
        match command {
            Commands::S3 { s3_command } => match s3_command {
                S3Commands::Upload {
                    local_file,
                    bucket,
                    path,
                } => {
                    s3_credentials_or_exit();
                    let local = local_file.to_string_lossy();
                    match oneio::s3_upload(&bucket, &path, &local) {
                        Ok(_) => println!("uploaded to s3://{bucket}/{path}"),
                        Err(e) => {
                            eprintln!("upload error: {e}");
                            exit(1);
                        }
                    }
                }

                S3Commands::Download {
                    bucket,
                    path,
                    outfile: local_outfile,
                } => {
                    s3_credentials_or_exit();
                    let local_path = match local_outfile {
                        Some(p) => p.to_string_lossy().into_owned(),
                        None => path
                            .split('/')
                            .next_back()
                            .unwrap_or("downloaded_file")
                            .to_string(),
                    };
                    let s3_url = format!("s3://{bucket}/{path}");
                    let result = if use_progress {
                        download_with_progress(
                            &oneio,
                            &s3_url,
                            &local_path,
                            &format!("s3://{bucket}/{path}"),
                        )
                    } else {
                        oneio::s3_download(&bucket, &path, &local_path)
                            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
                    };
                    match result {
                        Ok(_) => println!("downloaded s3://{bucket}/{path} to {local_path}"),
                        Err(e) => {
                            eprintln!("download error: {e}");
                            exit(1);
                        }
                    }
                }

                S3Commands::List {
                    bucket,
                    prefix,
                    delimiter,
                    dirs,
                } => {
                    s3_credentials_or_exit();
                    match oneio::s3_list(&bucket, &prefix, delimiter, dirs) {
                        Ok(paths) => paths.iter().for_each(|p| println!("{p}")),
                        Err(e) => {
                            eprintln!("list error: {e}");
                            exit(1);
                        }
                    }
                }
            },

            Commands::Digest { file } => {
                let path = file.to_string_lossy();
                match oneio::get_sha256_digest(&path) {
                    Ok(digest) => println!("{digest}"),
                    Err(e) => {
                        eprintln!("digest error: {e}");
                        exit(1);
                    }
                }
            }
        }
        return;
    }

    // Default: read FILE
    let path_string = cli.file.as_deref().unwrap().to_string_lossy().into_owned();
    let path = path_string.as_str();

    if cli.download {
        let out_path = match outfile {
            Some(p) => p.to_string_lossy().into_owned(),
            None => path
                .split('/')
                .next_back()
                .unwrap_or("output.txt")
                .to_string(),
        };
        let result = if use_progress {
            download_with_progress(&oneio, path, &out_path, path)
        } else {
            oneio
                .download(path, &out_path)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
        };
        match result {
            Ok(_) => println!("downloaded to {out_path}"),
            Err(e) => {
                eprintln!("download error: {e}");
                exit(1);
            }
        }
        return;
    }

    // Reader mode: cache > compression override > auto-detect
    let reader_result = if let Some(dir) = cli.cache_dir {
        oneio.get_cache_reader(path, &dir, cli.cache_file, cli.cache_force)
    } else if let Some(compression) = cli.compression {
        oneio.get_reader_with_type(path, &compression)
    } else {
        oneio.get_reader(path)
    };

    let reader = Box::new(BufReader::new(match reader_result {
        Ok(r) => r,
        Err(e) => {
            eprintln!("cannot open {path}: {e}");
            exit(1);
        }
    }));

    let mut stdout = std::io::stdout();
    let mut count_lines = 0usize;
    let mut count_chars = 0usize;

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                eprintln!("read error on {path}: {e}");
                exit(1);
            }
        };
        if !cli.stats {
            if let Err(e) = writeln!(stdout, "{line}") {
                if e.kind() != std::io::ErrorKind::BrokenPipe {
                    eprintln!("{e}");
                    exit(1);
                }
                exit(0);
            }
        }
        count_chars += line.chars().count();
        count_lines += 1;
    }

    if cli.stats {
        println!("lines: \t {count_lines}");
        println!("chars: \t {count_chars}");
    }
}
