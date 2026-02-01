import json

# Load the tree
with open('/tmp/devflow_tree.json', 'r') as f:
    tree = json.load(f)

total_size = tree['total_size']
total_files = tree['total_files']

# Rough token estimation
# 1 character ≈ 0.25-0.3 tokens for code
# Using conservative 0.3
estimated_tokens = int(total_size * 0.3)

# Grok context window
MAX_TOKENS = 2_000_000

print(f"📊 Content Size Analysis")
print(f"=" * 50)
print(f"Total Files: {total_files}")
print(f"Total Size: {total_size:,} bytes ({total_size/1024/1024:.2f} MB)")
print(f"Estimated Tokens: {estimated_tokens:,}")
print(f"Max Context Window: {MAX_TOKENS:,}")
print()
print(f"Usage: {estimated_tokens/MAX_TOKENS*100:.1f}% of context window")
print()

if estimated_tokens < MAX_TOKENS * 0.5:
    print("✅ RECOMMENDATION: Use Context Stuffing")
    print("   - Content easily fits in 2M window")
    print("   - Simpler implementation")
    print("   - Lower latency")
    print("   - No vector DB needed")
elif estimated_tokens < MAX_TOKENS * 0.8:
    print("⚠️  RECOMMENDATION: Consider Context Stuffing with pruning")
    print("   - Close to limit but manageable")
    print("   - Can exclude tests, docs, or generated files")
elif estimated_tokens < MAX_TOKENS:
    print("⚠️  RECOMMENDATION: Start with RAG")
    print("   - Near the limit")
    print("   - Better to use semantic search")
else:
    print("❌ RECOMMENDATION: Full RAG Required")
    print("   - Exceeds 2M token limit")
    print("   - Must use LanceDB + embeddings")
