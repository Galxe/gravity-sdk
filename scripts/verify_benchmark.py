#!/usr/bin/env python3
import sys
import re

def parse_log(log_path):
    """
    Parses the log file to find the specific benchmark summary table and verify success metrics.
    Looking for a table entry like:
    │ Progress        ┆ 30.0K/30.2K ┆ TPS           ┆ 100.1 │
    """
    try:
        with open(log_path, 'r', encoding='utf-8') as f:
            content = f.read()
    except FileNotFoundError:
        print(f"Error: Log file not found at {log_path}")
        return False
    except Exception as e:
        print(f"Error reading log file: {e}")
        return False

    # Regex to find the Progress line in the table
    # Matches: │ Progress        ┆ 123.4K/123.4K
    # We want to extract the first number (numerator)
    progress_pattern = re.compile(r"│\s*Progress\s*┆\s*([\d\.]+[KkMm]?)\s*/\s*([\d\.]+[KkMm]?)")
    
    # We scan specifically for the LAST occurrence of the table in the log, 
    # but scanning line by line and keeping the last match is also fine.
    
    last_progress_val = 0.0
    found_any = False

    for line in content.splitlines():
        match = progress_pattern.search(line)
        if match:
            # Parse the numerator (e.g. "30.0K")
            progress_str = match.group(1).upper()
            
            # Convert K/M suffixes
            multiplier = 1.0
            if progress_str.endswith('K'):
                multiplier = 1000.0
                progress_str = progress_str[:-1]
            elif progress_str.endswith('M'):
                multiplier = 1000000.0
                progress_str = progress_str[:-1]
            
            try:
                val = float(progress_str) * multiplier
                last_progress_val = val
                found_any = True
            except ValueError:
                continue

    if not found_any:
        print("Failure: Could not find 'Progress' metric in the log.")
        return False

    print(f"Last reported progress: {last_progress_val}")

    # Success criteria: Progress > 0
    if last_progress_val > 0:
        print("Success: Benchmark made progress.")
        return True
    else:
        print("Failure: Progress was 0.")
        return False

def main():
    if len(sys.argv) < 2:
        print("Usage: verify_benchmark.py <log_file>")
        sys.exit(1)

    log_file = sys.argv[1]
    if parse_log(log_file):
        sys.exit(0)
    else:
        sys.exit(1)

if __name__ == "__main__":
    main()
