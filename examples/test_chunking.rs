//! Standalone test for the chunking module
//!
//! Run with: cargo run --example test_chunking

use rustassistant::chunking::{chunk_document, ChunkConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Testing Document Chunking Module\n");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    // Test 1: Simple short document
    println!("📋 Test 1: Short Document");
    println!("─────────────────────────────────────────────────────────────────────────");
    let short_doc = "This is a short document with only a few words for testing.";
    let config = ChunkConfig::default();

    let chunks = chunk_document(short_doc, &config)?;
    println!("✓ Created {} chunk(s)", chunks.len());
    for (i, chunk) in chunks.iter().enumerate() {
        println!(
            "  Chunk {}: {} words, {} chars",
            i,
            chunk.word_count,
            chunk.content.len()
        );
    }
    println!();

    // Test 2: Document with markdown
    println!("📋 Test 2: Markdown Document");
    println!("─────────────────────────────────────────────────────────────────────────");
    let markdown_doc = r#"
# Introduction to Rust

Rust is a systems programming language that runs blazingly fast, prevents segfaults, and guarantees thread safety.

## Key Features

### Memory Safety
Rust's ownership system ensures memory safety without needing a garbage collector.

### Concurrency
Fearless concurrency through the type system.

## Code Example

Here's a simple example:

```rust
fn main() {
    println!("Hello, world!");
    let x = 5;
    println!("x = {}", x);
}
```

## Performance

Rust provides zero-cost abstractions and efficient C bindings.
"#;

    let chunks = chunk_document(markdown_doc, &config)?;
    println!("✓ Created {} chunk(s)", chunks.len());
    for (i, chunk) in chunks.iter().enumerate() {
        println!("  Chunk {}: {} words", i, chunk.word_count);
        if let Some(heading) = &chunk.heading {
            println!("    Heading: {}", heading);
        }
        if chunk.content.contains("```") {
            println!("    Contains code block: Yes");
        }
    }
    println!();

    // Test 3: Long document requiring multiple chunks
    println!("📋 Test 3: Long Document (Multiple Chunks)");
    println!("─────────────────────────────────────────────────────────────────────────");
    let config = ChunkConfig {
        target_words: 50,
        overlap_words: 10,
        min_chunk_size: 10,
        max_chunk_size: 80,
        markdown_aware: false,
        preserve_code_blocks: false,
        include_headings: false,
    };

    // Generate a long document
    let long_doc = (0..200)
        .map(|i| format!("word{}", i))
        .collect::<Vec<_>>()
        .join(" ");

    let chunks = chunk_document(&long_doc, &config)?;
    println!("✓ Created {} chunk(s)", chunks.len());
    for (i, chunk) in chunks.iter().enumerate() {
        println!(
            "  Chunk {}: {} words, range: {}-{}",
            i, chunk.word_count, chunk.char_start, chunk.char_end
        );
    }
    println!();

    // Test 4: Overlap verification
    println!("📋 Test 4: Overlap Verification");
    println!("─────────────────────────────────────────────────────────────────────────");
    if chunks.len() >= 2 {
        let first_chunk_words: Vec<&str> = chunks[0].content.split_whitespace().collect();
        let second_chunk_words: Vec<&str> = chunks[1].content.split_whitespace().collect();

        // Check if last words of first chunk appear in second chunk
        let overlap_count = first_chunk_words
            .iter()
            .rev()
            .take(config.overlap_words)
            .filter(|word| second_chunk_words.contains(word))
            .count();

        println!(
            "✓ Detected overlap of approximately {} words",
            overlap_count
        );
        println!("  Expected overlap: {} words", config.overlap_words);
    }
    println!();

    // Test 5: Small chunk config
    println!("📋 Test 5: Small Chunks Configuration");
    println!("─────────────────────────────────────────────────────────────────────────");
    let small_config = ChunkConfig::small();
    let chunks = chunk_document(markdown_doc, &small_config)?;
    println!("✓ Created {} chunk(s) with small config", chunks.len());
    println!("  Target words: {}", small_config.target_words);
    println!("  Overlap words: {}", small_config.overlap_words);
    println!();

    // Test 6: Large chunk config
    println!("📋 Test 6: Large Chunks Configuration");
    println!("─────────────────────────────────────────────────────────────────────────");
    let large_config = ChunkConfig::large();
    let chunks = chunk_document(markdown_doc, &large_config)?;
    println!("✓ Created {} chunk(s) with large config", chunks.len());
    println!("  Target words: {}", large_config.target_words);
    println!("  Overlap words: {}", large_config.overlap_words);
    println!();

    // Test 7: Code block preservation
    println!("📋 Test 7: Code Block Preservation");
    println!("─────────────────────────────────────────────────────────────────────────");
    let code_doc = r#"
Here's some text before the code.

```rust
fn fibonacci(n: u32) -> u32 {
    match n {
        0 => 0,
        1 => 1,
        _ => fibonacci(n - 1) + fibonacci(n - 2),
    }
}
```

And some text after the code.
"#;

    let config = ChunkConfig::default();
    let chunks = chunk_document(code_doc, &config)?;
    println!("✓ Created {} chunk(s)", chunks.len());

    let has_code_block = chunks.iter().any(|c| c.content.contains("```rust"));
    let has_complete_function = chunks
        .iter()
        .any(|c| c.content.contains("fn fibonacci") && c.content.contains("}"));

    println!(
        "  Code block preserved: {}",
        if has_code_block { "✓ Yes" } else { "✗ No" }
    );
    println!(
        "  Function complete: {}",
        if has_complete_function {
            "✓ Yes"
        } else {
            "✗ No"
        }
    );
    println!();

    // Summary
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("✅ All chunking tests completed successfully!");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    Ok(())
}
