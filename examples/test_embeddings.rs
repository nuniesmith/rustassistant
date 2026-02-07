//! Test Embedding Generation Example
//!
//! This example demonstrates the embedding generation capabilities:
//! 1. Initialize the embedding generator
//! 2. Generate embeddings for sample texts
//! 3. Calculate similarity between embeddings
//! 4. Test different models and configurations

use rustassistant::embeddings::{EmbeddingConfig, EmbeddingGenerator, EmbeddingModelType};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing for debug output
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("🧪 Testing Embedding Generation Module\n");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    // Test 1: Single embedding generation
    println!("📋 Test 1: Single Embedding Generation");
    println!("─────────────────────────────────────────────────────────────────────────");
    test_single_embedding().await?;
    println!();

    // Test 2: Batch embedding generation
    println!("📋 Test 2: Batch Embedding Generation");
    println!("─────────────────────────────────────────────────────────────────────────");
    test_batch_embeddings().await?;
    println!();

    // Test 3: Similarity calculation
    println!("📋 Test 3: Similarity Calculation");
    println!("─────────────────────────────────────────────────────────────────────────");
    test_similarity().await?;
    println!();

    // Test 4: Different models
    println!("📋 Test 4: Model Comparison");
    println!("─────────────────────────────────────────────────────────────────────────");
    test_models().await?;
    println!();

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("✅ All embedding tests completed successfully!");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    Ok(())
}

async fn test_single_embedding() -> anyhow::Result<()> {
    let config = EmbeddingConfig::default();
    let generator = EmbeddingGenerator::new(config)?;

    let text = "The quick brown fox jumps over the lazy dog";
    let embedding = generator.embed(text).await?;

    println!("✓ Generated embedding for text:");
    println!("  Text: \"{}\"", text);
    println!("  Model: {}", embedding.model);
    println!("  Dimension: {}", embedding.dimension);
    println!("  Vector length: {}", embedding.vector.len());
    println!(
        "  First 5 values: {:?}",
        &embedding.vector[..5.min(embedding.vector.len())]
    );

    Ok(())
}

async fn test_batch_embeddings() -> anyhow::Result<()> {
    let config = EmbeddingConfig::default();
    let generator = EmbeddingGenerator::new(config)?;

    let texts = vec![
        "Rust is a systems programming language",
        "Python is great for data science",
        "JavaScript runs in the browser",
        "Go is designed for concurrency",
        "C++ offers low-level control",
    ];

    let embeddings = generator.embed_batch(&texts).await?;

    println!("✓ Generated {} embeddings", embeddings.len());
    for (idx, (text, emb)) in texts.iter().zip(embeddings.iter()).enumerate() {
        println!("  [{}] \"{}\"", idx + 1, text);
        println!(
            "      Dimension: {}, Norm: {:.4}",
            emb.dimension,
            emb.vector.iter().map(|x| x * x).sum::<f32>().sqrt()
        );
    }

    Ok(())
}

async fn test_similarity() -> anyhow::Result<()> {
    let config = EmbeddingConfig::default();
    let generator = EmbeddingGenerator::new(config)?;

    let texts = vec![
        "The cat sits on the mat",
        "A feline rests on the rug",
        "Dogs are great pets",
        "The weather is sunny today",
    ];

    let embeddings = generator.embed_batch(&texts).await?;

    println!("✓ Similarity matrix:");
    println!("  Texts:");
    for (idx, text) in texts.iter().enumerate() {
        println!("    [{}] \"{}\"", idx, text);
    }
    println!("\n  Similarities:");

    for i in 0..embeddings.len() {
        for j in (i + 1)..embeddings.len() {
            let sim = embeddings[i].cosine_similarity(&embeddings[j])?;
            println!("    [{} ↔ {}]: {:.4}", i, j, sim);
        }
    }

    // Highlight most similar pair
    let mut max_sim = 0.0;
    let mut max_pair = (0, 0);
    for i in 0..embeddings.len() {
        for j in (i + 1)..embeddings.len() {
            let sim = embeddings[i].cosine_similarity(&embeddings[j])?;
            if sim > max_sim {
                max_sim = sim;
                max_pair = (i, j);
            }
        }
    }

    println!("\n  Most similar texts:");
    println!("    \"{}\"", texts[max_pair.0]);
    println!("    \"{}\"", texts[max_pair.1]);
    println!("    Similarity: {:.4}", max_sim);

    Ok(())
}

async fn test_models() -> anyhow::Result<()> {
    let models = vec![
        (EmbeddingModelType::BGESmallENV15, "BGE-small (fast, 384D)"),
        (
            EmbeddingModelType::AllMiniLML6V2,
            "MiniLM (very fast, 384D)",
        ),
    ];

    let text = "Rust programming language embeddings";

    for (model_type, description) in models {
        let config = EmbeddingConfig {
            model_name: model_type,
            batch_size: 32,
            show_download_progress: false,
            cache_dir: None,
        };

        let generator = EmbeddingGenerator::new(config)?;
        let embedding = generator.embed(text).await?;

        println!("✓ {}", description);
        println!("  Model: {}", embedding.model);
        println!("  Dimension: {}", embedding.dimension);
        println!(
            "  Vector norm: {:.4}",
            embedding.vector.iter().map(|x| x * x).sum::<f32>().sqrt()
        );
    }

    Ok(())
}
