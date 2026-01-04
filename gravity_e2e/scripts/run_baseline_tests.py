#!/usr/bin/env python3
"""
Analyze test files for code patterns before refactoring

This script analyzes test files to establish a baseline of code patterns
before the code consolidation refactoring begins.
"""

import os
import sys
import json
import re
from pathlib import Path
from typing import Dict, List, Tuple
from datetime import datetime

# Add parent directory to path
sys.path.insert(0, str(Path(__file__).parent.parent))

TEST_DIR = Path(__file__).parent.parent / "gravity_e2e" / "tests" / "test_cases"
RESULTS_FILE = Path(__file__).parent.parent / "code_analysis_baseline.json"


def discover_tests() -> List[Path]:
    """Discover all test files"""
    tests = []

    for test_file in TEST_DIR.glob("test_*.py"):
        # Skip test files that might require special setup
        if test_file.name in [
            "test_cross_chain_deposit.py",  # May need specific setup
            "test_zero_balance_env.py",      # Special environment
        ]:
            print(f"⚠️  Skipping {test_file.name} (requires special setup)")
            continue

        tests.append(test_file)

    return sorted(tests)


def run_single_test(test_path: Path) -> Tuple[bool, str, float]:
    """
    Run a single test file

    Returns:
        Tuple of (success, output, duration)
    """
    start_time = time.time()

    try:
        # Run the test using Python's -m flag to ensure proper imports
        result = subprocess.run(
            [sys.executable, "-m", "pytest", str(test_path), "-v", "--tb=short"],
            capture_output=True,
            text=True,
            timeout=300  # 5 minute timeout per test
        )

        duration = time.time() - start_time
        success = result.returncode == 0
        output = result.stdout + "\n" + result.stderr

        return success, output, duration

    except subprocess.TimeoutExpired:
        duration = time.time() - start_time
        return False, f"Test timed out after 300 seconds", duration
    except Exception as e:
        duration = time.time() - start_time
        return False, f"Error running test: {e}", duration


def main():
    """Main function to run all baseline tests"""
    print("🔍 Running baseline tests before refactoring...")
    print(f"Timestamp: {datetime.now().isoformat()}")
    print(f"Test directory: {TEST_DIR}")
    print()

    # Discover tests
    tests = discover_tests()
    if not tests:
        print("❌ No test files found!")
        return 1

    print(f"Found {len(tests)} test files:")
    for test in tests:
        print(f"  - {test.name}")
    print()

    # Run tests
    results = {
        "timestamp": datetime.now().isoformat(),
        "total_tests": len(tests),
        "results": []
    }

    passed = 0
    failed = 0

    for i, test_path in enumerate(tests, 1):
        print(f"[{i}/{len(tests)}] Running {test_path.name}...", end=" ")

        success, output, duration = run_single_test(test_path)

        if success:
            print(f"✅ PASSED ({duration:.2f}s)")
            passed += 1
        else:
            print(f"❌ FAILED ({duration:.2f}s)")
            failed += 1

        # Store result
        results["results"].append({
            "test_name": test_path.name,
            "success": success,
            "duration": duration,
            "output": output
        })

        # Print brief error summary on failure
        if not success:
            lines = output.strip().split('\n')
            for line in lines[-10:]:  # Last 10 lines
                if 'ERROR' in line or 'FAILED' in line:
                    print(f"    {line}")

    # Summary
    print("\n" + "="*50)
    print("BASELINE TEST SUMMARY")
    print("="*50)
    print(f"Total tests: {len(tests)}")
    print(f"Passed: {passed}")
    print(f"Failed: {failed}")
    print(f"Pass rate: {passed/len(tests)*100:.1f}%")

    if failed > 0:
        print("\n❌ Failed tests:")
        for result in results["results"]:
            if not result["success"]:
                print(f"  - {result['test_name']}")

    # Save results
    with open(RESULTS_FILE, 'w') as f:
        json.dump(results, f, indent=2)

    print(f"\n📊 Results saved to: {RESULTS_FILE}")

    # Return appropriate exit code
    return 0 if failed == 0 else 1


if __name__ == "__main__":
    sys.exit(main())