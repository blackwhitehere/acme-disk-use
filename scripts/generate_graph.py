#!/usr/bin/env python3
import re
import sys

def parse_results(filename):
    results = {}
    with open(filename, 'r') as f:
        content = f.read()
        
    # Use regex to find values
    # Rust (cold cache)            4229.38
    rust_cold_match = re.search(r"Rust \(cold cache\)\s+([\d\.]+)", content)
    if rust_cold_match:
        results["Rust (Cold)"] = float(rust_cold_match.group(1))
        
    rust_warm_match = re.search(r"Rust \(warm cache\)\s+([\d\.]+)", content)
    if rust_warm_match:
        results["Rust (Warm)"] = float(rust_warm_match.group(1))
        
    # du                           3987.56
    # We need to be careful not to match "Method" line or other things
    # Look for line starting with "du" followed by spaces and a number
    du_match = re.search(r"^du\s+([\d\.]+)", content, re.MULTILINE)
    if du_match:
        results["du"] = float(du_match.group(1))
        
    return results

def generate_svg(results, output_file):
    order = ["du", "Rust (Cold)", "Rust (Warm)"]
    
    # Config
    width = 600
    bar_height = 40
    gap = 20
    margin_left = 150
    margin_right = 50
    margin_top = 50
    margin_bottom = 30
    
    # Handle empty results
    if not results:
        print("No results to graph")
        return

    max_val = max(results.values())
    if max_val == 0:
        max_val = 1
        
    scale = (width - margin_left - margin_right) / max_val
    
    height = margin_top + (bar_height + gap) * len(order) + margin_bottom
    
    svg = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}">']
    svg.append(f'<rect width="100%" height="100%" fill="white"/>')
    svg.append(f'<text x="{width/2}" y="30" text-anchor="middle" font-family="sans-serif" font-size="16" font-weight="bold">Disk Usage Scan Time (Lower is Better)</text>')
    
    y = margin_top
    colors = {"du": "#95a5a6", "Rust (Cold)": "#3498db", "Rust (Warm)": "#2ecc71"}
    
    for name in order:
        val = results.get(name, 0)
        bar_width = val * scale
        color = colors.get(name, "#333")
        
        # Label
        svg.append(f'<text x="{margin_left - 10}" y="{y + bar_height/2 + 5}" text-anchor="end" font-family="sans-serif" font-size="14">{name}</text>')
        
        # Bar
        svg.append(f'<rect x="{margin_left}" y="{y}" width="{bar_width}" height="{bar_height}" fill="{color}" rx="4"/>')
        
        # Value label
        svg.append(f'<text x="{margin_left + bar_width + 10}" y="{y + bar_height/2 + 5}" font-family="sans-serif" font-size="12">{val:.1f} ms</text>')
        
        y += bar_height + gap
        
    svg.append('</svg>')
    
    with open(output_file, 'w') as f:
        f.write('\n'.join(svg))

if __name__ == "__main__":
    if len(sys.argv) < 3:
        print("Usage: generate_graph.py <results_file> <output_svg>")
        sys.exit(1)
        
    results = parse_results(sys.argv[1])
    print(f"Parsed results: {results}")
    
    if not results:
        print("No results found!")
        sys.exit(1)
        
    generate_svg(results, sys.argv[2])
    print(f"Generated {sys.argv[2]}")
