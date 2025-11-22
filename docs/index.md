# User Guide

## Introduction

`acme-disk-use` is a high-performance disk usage analyzer written in Rust. It is designed to be a faster alternative to the standard `du` command, particularly for large directory structures, by leveraging caching and parallel processing.

## Why use this over `du`?

The standard `du` command is reliable but can be slow on large directories because it has to traverse the entire filesystem tree every time it runs. `acme-disk-use` improves upon this in several ways:

1.  **Caching**: We store the size of directories in a cache. On subsequent runs, if a directory's modification time hasn't changed, we can skip traversing it entirely and use the cached value. This leads to massive speedups (often 100x or more) for repeated scans. We also make sure recusivly nested sub-directories are checked for their mtime modification.
2.  **Parallelism**: Rust's `rayon` library allows us to traverse independent subdirectories in parallel, utilizing all available CPU cores.

## Write Pattern Benefits

This tool is particularly beneficial for applications or workflows with the following "Write Pattern":

*   **Mostly Append-Only**: Directories where files are added but rarely modified or deleted.
*   **Deeply Nested Structures**: Paths where multiple independent processes populate sub-directories under the same root path
*   **Frequent Checks**: Scenarios where you need to check disk usage frequently to control disk space usage growth.
In these cases, the cache hit rate is high, and `acme-disk-use` provides near-instant results after the initial warm-up scan.

## Benchmark

See `benches` dir for benchmark results.

## Installation

```bash
cargo install --path .
```

## Usage

```bash
acme-disk-use [DIRECTORY]
```

See `acme-disk-use --help` for more options.
