# Justfile

# Run benchmarks and generate graph
benchmark:
    cargo build --release
    python3 benches/benchmark.py
    python3 benches/generate_graph.py benches/benchmark_results.json docs/benchmark_graph.svg
    @echo "Graph generated at docs/benchmark_graph.svg"
