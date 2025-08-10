#!/usr/bin/env rust-script
//! Progress Tracking Example
//!
//! This example demonstrates how to use OneIO's progress tracking feature
//! to monitor file download and reading progress. Progress tracking works
//! with both known and unknown file sizes.

use std::io::Read;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("OneIO Progress Tracking Examples\n");

    // Example 1: Basic progress tracking with local file
    println!("=== Example 1: Local File Progress ===");
    basic_local_progress()?;

    // Example 2: Progress with human-readable formatting
    println!("\n=== Example 2: Formatted Progress Display ===");
    formatted_progress()?;

    // Example 3: Progress with percentage calculation
    println!("\n=== Example 3: Percentage Progress ===");
    percentage_progress()?;

    // Example 4: Handling files without size information
    println!("\n=== Example 4: Unknown Size Progress ===");
    unknown_size_example()?;

    Ok(())
}

fn basic_local_progress() -> Result<(), Box<dyn std::error::Error>> {
    let (mut reader, total_size) =
        oneio::get_reader_with_progress("tests/test_data.txt.gz", |bytes_read, total_bytes| {
            println!("Progress: {}/{} bytes", bytes_read, total_bytes);
        })?;

    println!("File size: {} bytes (compressed)", total_size);

    let mut content = String::new();
    reader.read_to_string(&mut content)?;
    println!("Decompressed content: {} bytes", content.len());

    Ok(())
}

fn formatted_progress() -> Result<(), Box<dyn std::error::Error>> {
    fn format_bytes(bytes: u64) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
        let mut size = bytes as f64;
        let mut unit_idx = 0;

        while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
            size /= 1024.0;
            unit_idx += 1;
        }

        format!("{:.2} {}", size, UNITS[unit_idx])
    }

    let (mut reader, total_size) =
        oneio::get_reader_with_progress("tests/test_data.txt.bz2", |bytes_read, total_bytes| {
            if total_bytes > 0 {
                print!(
                    "\rProgress: {} / {} ",
                    format_bytes(bytes_read),
                    format_bytes(total_bytes)
                );
            } else {
                print!("\rDownloaded: {} ", format_bytes(bytes_read));
            }
            use std::io::Write;
            std::io::stdout().flush().unwrap();
        })?;

    println!("Reading {} file...", format_bytes(total_size));

    let mut buffer = vec![0; 512];
    while reader.read(&mut buffer)? > 0 {
        // Simulate some processing time
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    println!("\nComplete!");

    Ok(())
}

fn percentage_progress() -> Result<(), Box<dyn std::error::Error>> {
    let (mut reader, total_size) =
        oneio::get_reader_with_progress("tests/test_data.txt.lz4", |bytes_read, total_bytes| {
            if total_bytes > 0 {
                let percentage = (bytes_read as f64 / total_bytes as f64) * 100.0;
                print!(
                    "\rProgress: {:.1}% ({}/{})",
                    percentage, bytes_read, total_bytes
                );
            } else {
                print!("\rDownloaded: {} bytes (size unknown)", bytes_read);
            }
            use std::io::Write;
            std::io::stdout().flush().unwrap();
        })?;

    if total_size > 0 {
        println!("Starting download of {} bytes...", total_size);
    } else {
        println!("Starting download of unknown size...");
    }

    let mut content = Vec::new();
    reader.read_to_end(&mut content)?;

    println!("\nDownload complete! Read {} bytes total", content.len());

    Ok(())
}

fn unknown_size_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing progress tracking with streaming endpoint (unknown size)...");

    // Now progress tracking works even without Content-Length!
    let (mut reader, total_size) = oneio::get_reader_with_progress(
        "https://httpbin.org/stream/3", // Streaming endpoint without Content-Length
        |bytes_read, total_bytes| {
            if total_bytes > 0 {
                let percentage = (bytes_read as f64 / total_bytes as f64) * 100.0;
                print!("\rProgress: {:.1}% ({}/{})", percentage, bytes_read, total_bytes);
            } else {
                print!("\rDownloaded: {} bytes (size unknown)", bytes_read);
            }
        },
    )?;

    if total_size > 0 {
        println!("File size: {} bytes", total_size);
    } else {
        println!("File size: unknown (streaming)");
    }

    let mut content = String::new();
    reader.read_to_string(&mut content)?;
    
    println!("\n✓ Successfully read {} bytes with progress tracking!", content.len());
    println!("  Progress tracking now works even when total size is unknown!");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_examples_compile() {
        // Just ensure the examples compile
        assert!(true);
    }
}
