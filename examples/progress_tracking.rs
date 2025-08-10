#!/usr/bin/env rust-script
//! Progress Tracking Example
//!
//! This example demonstrates how to use OneIO's progress tracking feature
//! to monitor file download and reading progress.

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
    println!("\n=== Example 4: Error Handling ===");
    error_handling_example()?;

    Ok(())
}

fn basic_local_progress() -> Result<(), Box<dyn std::error::Error>> {
    let (mut reader, total_size) = oneio::get_reader_with_progress(
        "tests/test_data.txt.gz",
        |bytes_read, total_bytes| {
            println!("Progress: {}/{} bytes", bytes_read, total_bytes);
        }
    )?;

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

    let (mut reader, total_size) = oneio::get_reader_with_progress(
        "tests/test_data.txt.bz2", 
        |bytes_read, total_bytes| {
            print!("\rProgress: {} / {} ", 
                   format_bytes(bytes_read), 
                   format_bytes(total_bytes));
            use std::io::Write;
            std::io::stdout().flush().unwrap();
        }
    )?;

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
    let (mut reader, total_size) = oneio::get_reader_with_progress(
        "tests/test_data.txt.lz4",
        |bytes_read, total_bytes| {
            let percentage = (bytes_read as f64 / total_bytes as f64) * 100.0;
            print!("\rProgress: {:.1}% ({}/{})", percentage, bytes_read, total_bytes);
            use std::io::Write;
            std::io::stdout().flush().unwrap();
        }
    )?;

    println!("Starting download of {} bytes...", total_size);
    
    let mut content = Vec::new();
    reader.read_to_end(&mut content)?;
    
    println!("\nDownload complete! Read {} bytes total", content.len());
    
    Ok(())
}

fn error_handling_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing progress tracking with file that has no size info...");
    
    // Try to track progress on a streaming endpoint (will fail)
    match oneio::get_reader_with_progress(
        "https://httpbin.org/stream/5", // Streaming endpoint without Content-Length
        |bytes_read, total_bytes| {
            println!("Progress: {}/{}", bytes_read, total_bytes);
        }
    ) {
        Ok((mut reader, total_size)) => {
            println!("Unexpected success! Size: {}", total_size);
            let mut content = String::new();
            reader.read_to_string(&mut content)?;
            println!("Content: {}", content);
        }
        Err(oneio::OneIoError::NotSupported(msg)) => {
            println!("✓ Expected error: {}", msg);
            println!("  This is the correct behavior - progress tracking fails when size is unknown");
        }
        Err(e) => {
            println!("✗ Unexpected error: {:?}", e);
        }
    }
    
    println!("\nFalling back to regular reader without progress...");
    match oneio::get_reader("https://httpbin.org/stream/3") {
        Ok(mut reader) => {
            let mut content = String::new();
            reader.read_to_string(&mut content)?;
            println!("Successfully read {} bytes without progress tracking", content.len());
        }
        Err(e) => {
            println!("Failed to read without progress: {:?}", e);
        }
    }
    
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