#!/usr/bin/env python3
import re
import sys

def parse_results(filename):
    import json
    results = {}
    stats = {}
    try:
        with open(filename, 'r') as f:
            data = json.load(f)
            
        # Handle new format vs old format
        if isinstance(data, dict) and "results" in data:
            entries = data["results"]
            stats = data.get("stats", {})
        else:
            entries = data
            
        for entry in entries:
            name = entry['name']
            avg = entry['avg']
            
            if name == "Rust (cold cache)":
                results["Rust (Cold)"] = avg
            elif name == "Rust (warm cache)":
                results["Rust (Warm)"] = avg
            elif name == "du":
                results["du"] = avg
                
    except Exception as e:
        print(f"Error reading JSON: {e}")
        
    return results, stats

def generate_svg(results, stats, output_file):
    order = ["du", "Rust (Cold)", "Rust (Warm)"]
    
    # Config
    width = 600
    bar_height = 40
    gap = 20
    margin_left = 150
    margin_right = 50
    margin_top = 50
    margin_bottom = 30
    
    # Handle stats subtitle
    subtitle = ""
    if stats:
        files = stats.get("total_files", 0)
        size = stats.get("total_size", 0)
        
        # Format size
        unit = 'B'
        for u in ['B', 'KB', 'MB', 'GB', 'TB']:
            if size < 1024:
                unit = u
                break
            size /= 1024
            
        subtitle = f"Scanned {files:,} files ({size:.2f} {unit})"
        margin_top = 80 # Increase top margin for subtitle

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
    
    if subtitle:
        svg.append(f'<text x="{width/2}" y="55" text-anchor="middle" font-family="sans-serif" font-size="14" fill="#555">{subtitle}</text>')
    
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
        print("Usage: generate_graph.py <json_results_file> <output_svg>")
        sys.exit(1)
        
    results, stats = parse_results(sys.argv[1])
    print(f"Parsed results: {results}")
    print(f"Parsed stats: {stats}")
    
    if not results:
        print("No results found!")
        sys.exit(1)
        
    generate_svg(results, stats, sys.argv[2])
    print(f"Generated {sys.argv[2]}")
