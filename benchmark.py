#!/usr/bin/env python3
import argparse
import os
import shutil
import subprocess
import sys
import time
import statistics
import platform
from pathlib import Path

# Configuration
SCRIPT_DIR = Path(__file__).parent.absolute()
RUST_BIN = SCRIPT_DIR / "target" / "release" / "acme-disk-use"
BENCHMARK_DIR = SCRIPT_DIR / "benchmark_data"
CACHE_FILE = Path.home() / ".cache" / "acme-disk-use" / "cache.json"

# Colors
class Colors:
    RED = '\033[0;31m'
    GREEN = '\033[0;32m'
    YELLOW = '\033[1;33m'
    BLUE = '\033[0;34m'
    NC = '\033[0m'

def log_info(msg):
    print(f"{Colors.BLUE}[INFO]{Colors.NC} {msg}")

def log_success(msg):
    print(f"{Colors.GREEN}[SUCCESS]{Colors.NC} {msg}")

def log_warning(msg):
    print(f"{Colors.YELLOW}[WARNING]{Colors.NC} {msg}")

def log_error(msg):
    print(f"{Colors.RED}[ERROR]{Colors.NC} {msg}")

def create_nested_structure(base_dir, depth, files_per_dir, subdirs_per_dir, file_size):
    if depth < 0:
        return

    # Create files
    for i in range(1, files_per_dir + 1):
        file_path = base_dir / f"file_{i}.dat"
        # Efficiently create sparse or small files
        if file_size > 0:
            with open(file_path, "wb") as f:
                # Write random bytes if needed, or just zeros for speed if content doesn't matter for `du`
                # Using os.urandom is slower for massive counts, but requested "1 byte files".
                # For 1 byte, it's negligible.
                f.write(os.urandom(file_size))
        else:
            file_path.touch()

    # Create subdirs
    if depth > 0:
        for i in range(1, subdirs_per_dir + 1):
            subdir = base_dir / f"subdir_{i}"
            subdir.mkdir(exist_ok=True)
            create_nested_structure(subdir, depth - 1, files_per_dir, subdirs_per_dir, file_size)

def create_benchmark_data(depth, files_per_dir, subdirs_per_dir, file_size):
    log_info("Creating benchmark data...")
    log_info(f"  Depth: {depth}")
    log_info(f"  Files per dir: {files_per_dir}")
    log_info(f"  Subdirs per dir: {subdirs_per_dir}")
    log_info(f"  File size: {file_size} bytes")

    if BENCHMARK_DIR.exists():
        shutil.rmtree(BENCHMARK_DIR)
    BENCHMARK_DIR.mkdir(parents=True)

    create_nested_structure(BENCHMARK_DIR, depth, files_per_dir, subdirs_per_dir, file_size)

    # Count files
    total_files = sum(1 for _ in BENCHMARK_DIR.rglob('*') if _.is_file())
    total_dirs = sum(1 for _ in BENCHMARK_DIR.rglob('*') if _.is_dir())
    
    log_success(f"Created {total_files} files in {total_dirs} directories")

def run_command(cmd, cwd=None, capture_output=True):
    try:
        result = subprocess.run(
            cmd, 
            cwd=cwd, 
            shell=True, 
            check=False, 
            stdout=subprocess.PIPE if capture_output else None, 
            stderr=subprocess.PIPE if capture_output else None,
            text=True
        )
        return result
    except Exception as e:
        return None

def benchmark_func(name, cmd_func, runs=5):
    log_info(f"Benchmarking: {name} ({runs} runs)")
    times = []
    
    for _ in range(runs):
        start = time.perf_counter_ns()
        cmd_func()
        end = time.perf_counter_ns()
        elapsed_ms = (end - start) / 1_000_000
        times.append(elapsed_ms)
    
    avg = statistics.mean(times)
    median = statistics.median(times)
    min_val = min(times)
    max_val = max(times)
    
    return {
        "name": name,
        "avg": avg,
        "median": median,
        "min": min_val,
        "max": max_val
    }

def get_du_command():
    if platform.system() == "Darwin":
        return f"du -sk '{BENCHMARK_DIR}'" # macOS blocks
    else:
        return f"du -sb '{BENCHMARK_DIR}'" # GNU bytes

def verify_correctness():
    log_info("Verifying correctness...")
    
    # Reference (find + stat)
    # Using a simplified python approach for reference to avoid shell complexity
    ref_size = sum(f.stat().st_size for f in BENCHMARK_DIR.rglob('*') if f.is_file())
    log_info(f"Reference (Python walk): {ref_size} bytes")

    # Rust
    if CACHE_FILE.exists():
        CACHE_FILE.unlink()
    
    rust_res = run_command(f"'{RUST_BIN}' --ignore-cache --non-human-readable '{BENCHMARK_DIR}'")
    rust_size = 0
    if rust_res and rust_res.returncode == 0:
        # Parse "total size: 12345"
        import re
        match = re.search(r"total size: (\d+)", rust_res.stdout)
        if match:
            rust_size = int(match.group(1))
            log_info(f"Rust reports: {rust_size} bytes")
        else:
            log_error(f"Could not parse Rust output: {rust_res.stdout}")
    else:
        log_error("Rust binary failed to run")

    if rust_size == ref_size:
        log_success(f"Rust matches reference: {ref_size} bytes! ✓")
    else:
        log_warning(f"Mismatch! Ref: {ref_size}, Rust: {rust_size}")

def main():
    parser = argparse.ArgumentParser(description="Acme Disk Usage Benchmark Tool", formatter_class=argparse.RawDescriptionHelpFormatter)
    
    # Default scenario: 220K files
    # Depth 4, Subdirs 10, Files 20 => ~222K files
    parser.add_argument("--depth", type=int, default=4, help="Directory nesting depth")
    parser.add_argument("--files", type=int, default=20, help="Files per directory")
    parser.add_argument("--subdirs", type=int, default=10, help="Subdirectories per directory")
    parser.add_argument("--size", type=int, default=1, help="File size in bytes")
    parser.add_argument("--runs", type=int, default=5, help="Number of benchmark runs")
    parser.add_argument("--recreate", action="store_true", help="Force recreation of benchmark data")
    
    epilog = """
REFERENCE:
    This script replaces the old benchmark.sh and benchmark-reference.sh.
    
    DEFAULT SCENARIO (~220K files):
      Depth: 4
      Files/Dir: 20
      Subdirs/Dir: 10
      Total Files: ~222,220
    
    EXAMPLES:
      python3 benchmark.py                  # Run default benchmark
      python3 benchmark.py --runs 10        # Run 10 iterations
      python3 benchmark.py --recreate       # Force data regeneration
    """
    parser.epilog = epilog
    
    args = parser.parse_args()

    if not RUST_BIN.exists():
        log_info("Building release binary...")
        run_command("cargo build --release", cwd=SCRIPT_DIR)

    # Check if we need to create data
    should_create = False
    if args.recreate:
        should_create = True
    elif not BENCHMARK_DIR.exists():
        should_create = True
    else:
        # Check if empty
        if not any(BENCHMARK_DIR.iterdir()):
            should_create = True
        else:
            log_info("Benchmark data exists. Skipping creation (use --recreate to force).")

    if should_create:
        create_benchmark_data(args.depth, args.files, args.subdirs, args.size)

    verify_correctness()
    
    print("\n" + "="*60)
    print("                    BENCHMARK RESULTS")
    print("="*60 + "\n")
    
    # Warmup
    run_command(f"'{RUST_BIN}' --ignore-cache '{BENCHMARK_DIR}'")

    results = []
    
    # Rust Cold
    def run_rust_cold():
        if CACHE_FILE.exists():
            CACHE_FILE.unlink()
        run_command(f"'{RUST_BIN}' --ignore-cache '{BENCHMARK_DIR}'")
    
    results.append(benchmark_func("Rust (cold cache)", run_rust_cold, args.runs))

    # Rust Warm
    def run_rust_warm():
        run_command(f"'{RUST_BIN}' '{BENCHMARK_DIR}'")
    
    results.append(benchmark_func("Rust (warm cache)", run_rust_warm, args.runs))

    # DU
    du_cmd = get_du_command()
    def run_du():
        run_command(du_cmd)
    
    results.append(benchmark_func("du", run_du, args.runs))

    # Report
    print(f"{'Method':<25} {'Avg(ms)':>10} {'Median(ms)':>10} {'Min(ms)':>10} {'Max(ms)':>10}")
    print("-" * 65)
    for r in results:
        print(f"{r['name']:<25} {r['avg']:>10.2f} {r['median']:>10.2f} {r['min']:>10.2f} {r['max']:>10.2f}")
    print("="*60 + "\n")

    # Speedup
    rust_warm = next((r['avg'] for r in results if r['name'] == "Rust (warm cache)"), None)
    du_time = next((r['avg'] for r in results if r['name'] == "du"), None)
    
    if rust_warm and du_time and rust_warm > 0:
        speedup = du_time / rust_warm
        log_success(f"Rust (warm cache) is {speedup:.2f}x faster than du")

if __name__ == "__main__":
    main()
