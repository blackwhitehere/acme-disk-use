#!/usr/bin/env bash
set -euo pipefail

# Benchmark script comparing disk usage calculation methods
# Tests: Rust implementation vs du vs find+awk

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUST_BIN="$SCRIPT_DIR/target/release/acme-disk-use"
BENCHMARK_DIR="$SCRIPT_DIR/benchmark_data"
CACHE_FILE="$HOME/.cache/acme-disk-use/cache.json"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Print colored output
log_info() {
    echo -e "${BLUE}[INFO]${NC} $*"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $*"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $*"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $*"
}

# Create benchmark data structure
create_benchmark_data() {
    local depth=$1
    local files_per_dir=$2
    local subdirs_per_dir=$3
    local file_size=$4
    
    log_info "Creating benchmark data..."
    log_info "  Depth: $depth levels"
    log_info "  Files per directory: $files_per_dir"
    log_info "  Subdirectories per directory: $subdirs_per_dir"
    log_info "  File size: ~$file_size bytes"
    
    rm -rf "$BENCHMARK_DIR"
    mkdir -p "$BENCHMARK_DIR"
    
    create_nested_structure "$BENCHMARK_DIR" "$depth" "$files_per_dir" "$subdirs_per_dir" "$file_size"
    
    local total_files=$(find "$BENCHMARK_DIR" -type f | wc -l)
    local total_dirs=$(find "$BENCHMARK_DIR" -type d | wc -l)
    
    log_success "Created $total_files files in $total_dirs directories"
}

# Recursively create nested directory structure
create_nested_structure() {
    local base_dir=$1
    local depth=$2
    local files_per_dir=$3
    local subdirs_per_dir=$4
    local file_size=$5
    
    if [ "$depth" -le 0 ]; then
        return
    fi
    
    # Create files in current directory
    for i in $(seq 1 "$files_per_dir"); do
        head -c "$file_size" /dev/urandom > "$base_dir/file_$i.dat" 2>/dev/null
    done
    
    # Create subdirectories and recurse
    if [ "$depth" -gt 1 ]; then
        for i in $(seq 1 "$subdirs_per_dir"); do
            local subdir="$base_dir/subdir_$i"
            mkdir -p "$subdir"
            create_nested_structure "$subdir" $((depth - 1)) "$files_per_dir" "$subdirs_per_dir" "$file_size"
        done
    fi
}

# Benchmark function with timing
benchmark() {
    local name=$1
    local cmd=$2
    local runs=${3:-5}
    
    log_info "Benchmarking: $name (${runs} runs)" >&2
    
    local times=()
    local total=0
    
    for i in $(seq 1 "$runs"); do
        local start=$(date +%s%N)
        eval "$cmd" > /dev/null 2>&1
        local end=$(date +%s%N)
        local elapsed=$(( (end - start) / 1000000 )) # Convert to milliseconds
        times+=("$elapsed")
        total=$((total + elapsed))
    done
    
    local avg=$((total / runs))
    
    # Calculate median
    IFS=$'\n' sorted=($(printf '%s\n' "${times[@]}" | sort -n))
    unset IFS
    local median_idx=$((runs / 2))
    local median=${sorted[$median_idx]}
    
    # Calculate min/max
    local min=${sorted[0]}
    local max=${sorted[$((runs - 1))]}
    
    echo "$name,$avg,$median,$min,$max"
}

# Method 1: Our Rust implementation (cold cache)
rust_cold() {
    rm -f "$CACHE_FILE"
    "$RUST_BIN" --ignore-cache "$BENCHMARK_DIR" >/dev/null 2>&1
}

# Method 2: Our Rust implementation (warm cache)
rust_warm() {
    "$RUST_BIN" "$BENCHMARK_DIR" >/dev/null 2>&1
}

# Method 3: Standard du command (macOS compatible)
du_method() {
    if du -sb "$BENCHMARK_DIR" >/dev/null 2>&1; then
        # GNU du
        du -sb "$BENCHMARK_DIR" >/dev/null 2>&1
    else
        # BSD du (macOS)
        du -sk "$BENCHMARK_DIR" >/dev/null 2>&1
    fi
}

# Run all benchmarks
run_benchmarks() {
    local runs=${1:-5}
    
    log_info "Running benchmarks with $runs iterations each..."
    echo ""
    
    # Results header
    echo "Method,Avg(ms),Median(ms),Min(ms),Max(ms)"
    
    # Warm up the Rust binary cache
    "$RUST_BIN" --ignore-cache "$BENCHMARK_DIR" >/dev/null 2>&1
    
    # Run benchmarks
    benchmark "Rust (cold cache)" "rust_cold" "$runs"
    benchmark "Rust (warm cache)" "rust_warm" "$runs"
    benchmark "du" "du_method" "$runs"
}

# Verify correctness of all methods
verify_correctness() {
    log_info "Verifying correctness of all methods..."
    
    # Get reference value from find+stat (most reliable cross-platform)
    local reference_result=$(find "$BENCHMARK_DIR" -type f -exec stat -f%z {} \; 2>/dev/null | awk '{total += $1} END {print total}')
    if [ -z "$reference_result" ]; then
        # Try GNU stat format
        reference_result=$(find "$BENCHMARK_DIR" -type f -exec stat -c%s {} \; 2>/dev/null | awk '{total += $1} END {print total}')
    fi
    log_info "Reference (find+stat): $reference_result bytes"
    
    # Test Rust implementation
    rm -f "$CACHE_FILE"
    local rust_output=$("$RUST_BIN" --ignore-cache --non-human-readable "$BENCHMARK_DIR" 2>/dev/null)
    # Parse "Found X files, total size: Y" format
    local rust_result=$(echo "$rust_output" | grep -oE "total size: [0-9]+" | awk '{print $3}')
    log_info "Rust reports: $rust_result bytes"
    
    # Get du result (platform dependent, for reference only)
    local du_result
    if du -sb "$BENCHMARK_DIR" >/dev/null 2>&1; then
        # GNU du
        du_result=$(du -sb "$BENCHMARK_DIR" 2>/dev/null | awk '{print $1}')
        log_info "du -sb reports: $du_result bytes"
    else
        # BSD du (macOS) - reports 512-byte blocks
        local du_blocks=$(du -s "$BENCHMARK_DIR" 2>/dev/null | awk '{print $1}')
        du_result=$((du_blocks * 512))
        log_info "du -s reports: $du_result bytes (${du_blocks} blocks)"
    fi
    
    echo ""
    
    # Compare results against reference
    if [ -n "$rust_result" ] && [ -n "$reference_result" ]; then
        if [ "$rust_result" -eq "$reference_result" ]; then
            log_success "Rust matches reference: $reference_result bytes! ✓"
            return 0
        else
            log_warning "Methods disagree:"
            log_warning "  Reference:    $reference_result"
            log_warning "  Rust:         $rust_result"
            log_warning "  du:           ${du_result:-N/A} (block-based, may differ)"
            return 1
        fi
    else
        log_error "Failed to parse output from one or more methods"
        log_error "  Reference:    ${reference_result:-FAILED}"
        log_error "  Rust:         ${rust_result:-FAILED}"
        log_error "  du:           ${du_result:-FAILED}"
        return 1
    fi
}

# Test cache invalidation
test_cache_invalidation() {
    log_info "Testing cache invalidation..."
    
    # First scan (cold)
    rm -f "$CACHE_FILE"
    local output1=$("$RUST_BIN" --non-human-readable "$BENCHMARK_DIR" 2>&1)
    local time1=$(echo "$output1" | grep -oE "total size: [0-9]+" | awk '{print $3}')
    local files1=$(echo "$output1" | grep -oE "Found [0-9]+" | awk '{print $2}')
    
    # Second scan (warm - should use cache)
    local start=$(date +%s%N)
    "$RUST_BIN" "$BENCHMARK_DIR" >/dev/null 2>&1
    local end=$(date +%s%N)
    local warm_time=$(( (end - start) / 1000000 ))
    
    log_info "Warm cache scan took: ${warm_time}ms"
    
    # Add a new file
    echo "test" > "$BENCHMARK_DIR/new_file.txt"
    
    # Third scan (should detect change)
    local output3=$("$RUST_BIN" --non-human-readable "$BENCHMARK_DIR" 2>&1)
    local time3=$(echo "$output3" | grep -oE "total size: [0-9]+" | awk '{print $3}')
    local files3=$(echo "$output3" | grep -oE "Found [0-9]+" | awk '{print $2}')
    
    if [ -n "$files1" ] && [ -n "$files3" ] && [ "$files3" -gt "$files1" ]; then
        log_success "Cache invalidation working! Detected new file ($files1 -> $files3 files)."
    else
        log_warning "Cache invalidation unclear. Files: ${files1:-?} vs ${files3:-?}"
    fi
    
    # Clean up
    rm "$BENCHMARK_DIR/new_file.txt"
}

# Generate report
generate_report() {
    local results_file=$1
    
    log_info "Generating benchmark report..."
    
    echo ""
    echo "═══════════════════════════════════════════════════════════════"
    echo "                    BENCHMARK RESULTS"
    echo "═══════════════════════════════════════════════════════════════"
    echo ""
    
    # Parse and display results
    printf "%-25s %10s %10s %10s %10s\n" "Method" "Avg(ms)" "Median(ms)" "Min(ms)" "Max(ms)"
    echo "───────────────────────────────────────────────────────────────"
    
    while IFS=, read -r method avg median min max; do
        if [[ "$method" != "Method" ]]; then
            printf "%-25s %10s %10s %10s %10s\n" "$method" "$avg" "$median" "$min" "$max"
        fi
    done < "$results_file"
    
    echo "═══════════════════════════════════════════════════════════════"
    echo ""
    
    # Calculate speedup
    local rust_warm=$(awk -F, '/Rust \(warm cache\)/ {print $2}' "$results_file")
    local rust_cold=$(awk -F, '/Rust \(cold cache\)/ {print $2}' "$results_file")
    local du_time=$(awk -F, '/^du,/ {print $2}' "$results_file")
    
    if [ -n "$rust_warm" ] && [ -n "$du_time" ] && [ "$rust_warm" -gt 0 ]; then
        local speedup_vs_du=$(awk "BEGIN {printf \"%.2f\", $du_time / $rust_warm}")
        log_success "Rust (warm cache) is ${speedup_vs_du}x faster than du"
    fi
    
    if [ -n "$rust_cold" ] && [ -n "$du_time" ] && [ "$du_time" -gt 0 ]; then
        local ratio_cold=$(awk "BEGIN {printf \"%.2f\", $rust_cold / $du_time}")
        log_info "Rust (cold cache) is ${ratio_cold}x relative to du (includes cache write overhead)"
    fi
}

# Main function
main() {
    log_info "Acme Disk Usage Benchmark Suite"
    echo ""
    
    # Check if release binary exists
    if [ ! -f "$RUST_BIN" ]; then
        log_info "Building release binary..."
        cd "$SCRIPT_DIR"
        cargo build --release
    fi
    
    # Parse arguments
    local depth=${1:-4}
    local files_per_dir=${2:-10}
    local subdirs_per_dir=${3:-3}
    local file_size=${4:-1024}
    local runs=${5:-5}
    
    # Create benchmark data
    create_benchmark_data "$depth" "$files_per_dir" "$subdirs_per_dir" "$file_size"
    echo ""
    
    # Verify correctness
    verify_correctness
    echo ""
    
    # Test cache invalidation
    test_cache_invalidation
    echo ""
    
    # Run benchmarks
    local results_file=$(mktemp)
    run_benchmarks "$runs" | tee "$results_file"
    echo ""
    
    # Generate report
    generate_report "$results_file"
    
    # Cleanup
    rm -f "$results_file"
    
    log_info "Benchmark complete!"
    log_info "Benchmark data preserved at: $BENCHMARK_DIR"
}

# Show usage
usage() {
    cat << EOF
Usage: $0 [depth] [files_per_dir] [subdirs_per_dir] [file_size] [runs]

Arguments:
    depth            - Directory nesting depth (default: 4)
    files_per_dir    - Number of files per directory (default: 10)
    subdirs_per_dir  - Number of subdirectories per directory (default: 3)
    file_size        - Size of each file in bytes (default: 1024)
    runs             - Number of benchmark iterations (default: 5)

Examples:
    $0                              # Use defaults
    $0 5 20 4 2048 10              # Deep structure, 10 runs
    $0 3 5 2 512 3                 # Small structure, 3 runs

The script will:
  1. Create a nested directory structure with test data
  2. Verify all methods produce the same results
  3. Test cache invalidation
  4. Benchmark all methods and report results
EOF
}

# Handle help flag
if [[ "${1:-}" == "-h" ]] || [[ "${1:-}" == "--help" ]]; then
    usage
    exit 0
fi

# Run main
main "$@"
