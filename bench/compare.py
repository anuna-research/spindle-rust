#!/usr/bin/env python3
"""
Benchmark comparison: spindle-rust vs spindle-racket

Generates theories of various sizes and compares performance between
the Rust and Racket implementations.

Usage:
    python3 bench/compare.py [--tiers small,medium,large] [--iterations 5]
"""

import argparse
import json
import os
import random
import shutil
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Optional


@dataclass
class BenchmarkResult:
    """Result from a single benchmark run."""
    time_ms: float
    conclusions: int
    mode: str
    success: bool
    error: Optional[str] = None


@dataclass
class TierConfig:
    """Configuration for a benchmark tier."""
    name: str
    rules: int
    facts: int
    target_ms: int  # Target P95 time


# NFR-based tiers from spindle-racket
TIERS = {
    "small": TierConfig("Small", rules=100, facts=500, target_ms=50),
    "medium": TierConfig("Medium", rules=1000, facts=10000, target_ms=500),
    "large": TierConfig("Large", rules=10000, facts=100000, target_ms=10000),
    "chain": TierConfig("Chain", rules=100, facts=1, target_ms=50),  # Deep chain
    "conflict": TierConfig("Conflict", rules=500, facts=50, target_ms=100),  # Conflict-heavy
}


def generate_theory_dfl(num_facts: int, num_rules: int, seed: int = 42) -> str:
    """
    Generate a DFL theory string with specified number of facts and rules.

    Creates:
    - Ground facts
    - Mix of strict and defeasible rules
    - ~10% conflicting rules
    - Some superiority relations
    """
    random.seed(seed)
    lines = ["# Generated benchmark theory", ""]

    # Add facts
    for i in range(num_facts):
        lines.append(f"f{i}: >> fact{i}")

    lines.append("")

    # Add rules (mix of strict and defeasible)
    for i in range(num_rules):
        premise_idx = i % max(1, num_facts)
        premise = f"fact{premise_idx}"
        conclusion = f"conclusion{i}"

        # Alternate between strict (->) and defeasible (=>)
        if i % 2 == 0:
            lines.append(f"r{i}: {premise} -> {conclusion}")
        else:
            lines.append(f"r{i}: {premise} => {conclusion}")

    lines.append("")

    # Add conflicting rules (~10% of rules)
    num_conflicts = num_rules // 10
    for i in range(num_conflicts):
        premise_idx = i % max(1, num_facts)
        premise = f"fact{premise_idx}"
        target = f"conclusion{i * 2}"
        lines.append(f"conflict{i}: {premise} => ¬{target}")

    lines.append("")

    # Add superiority relations
    for i in range(min(num_conflicts, 5)):
        lines.append(f"r{i * 2} > conflict{i}")

    return "\n".join(lines)


def generate_chain_theory_dfl(chain_length: int) -> str:
    """
    Generate a chain theory where each rule depends on the previous conclusion.
    Tests deep reasoning chains.
    """
    lines = ["# Chain theory", ""]

    # Initial fact
    lines.append("f0: >> p0")
    lines.append("")

    # Chain of defeasible rules
    for i in range(chain_length):
        lines.append(f"r{i}: p{i} => p{i + 1}")

    return "\n".join(lines)


def generate_conflict_heavy_theory_dfl(num_chains: int, chain_depth: int) -> str:
    """
    Generate a theory with multiple parallel chains and conflicts at each level.
    Designed to stress-test conflict resolution.
    """
    lines = ["# Conflict-heavy theory", ""]

    for chain_id in range(num_chains):
        # Initial fact
        lines.append(f"f{chain_id}: >> start{chain_id}")

        for level in range(chain_depth):
            prev_lit = f"start{chain_id}" if level == 0 else f"c{chain_id}_l{level - 1}"
            curr_lit = f"c{chain_id}_l{level}"

            # Main rule
            lines.append(f"r{chain_id}_l{level}: {prev_lit} => {curr_lit}")

            # Conflicting rule
            lines.append(f"x{chain_id}_l{level}: {prev_lit} => ¬{curr_lit}")

            # Superiority
            lines.append(f"r{chain_id}_l{level} > x{chain_id}_l{level}")

        lines.append("")

    return "\n".join(lines)


def run_rust_benchmark(dfl_file: Path) -> BenchmarkResult:
    """Run spindle-rust on a DFL file and measure time."""
    script_dir = Path(__file__).parent
    rust_binary = script_dir.parent / "target" / "release" / "spindle"

    if not rust_binary.exists():
        # Try debug build
        rust_binary = script_dir.parent / "target" / "debug" / "spindle"

    if not rust_binary.exists():
        return BenchmarkResult(
            time_ms=0, conclusions=0, mode="standard",
            success=False, error="spindle binary not found. Run: cargo build --release"
        )

    cmd = [str(rust_binary), str(dfl_file)]

    try:
        start = time.perf_counter()
        result = subprocess.run(
            cmd, capture_output=True, text=True, timeout=300
        )
        elapsed_ms = (time.perf_counter() - start) * 1000

        if result.returncode != 0:
            return BenchmarkResult(
                time_ms=elapsed_ms, conclusions=0,
                mode="standard",
                success=False, error=result.stderr
            )

        # Count conclusions from output
        conclusions = sum(1 for line in result.stdout.split('\n')
                        if line.strip().startswith(('+D', '-D', '+d', '-d')))

        return BenchmarkResult(
            time_ms=elapsed_ms,
            conclusions=conclusions,
            mode="standard",
            success=True
        )
    except subprocess.TimeoutExpired:
        return BenchmarkResult(
            time_ms=300000, conclusions=0,
            mode="standard",
            success=False, error="Timeout (5 min)"
        )
    except Exception as e:
        return BenchmarkResult(
            time_ms=0, conclusions=0,
            mode="standard",
            success=False, error=str(e)
        )


def run_racket_benchmark(dfl_file: Path) -> BenchmarkResult:
    """Run spindle-racket on a DFL file and measure time."""
    script_dir = Path(__file__).parent
    racket_runner = script_dir / "racket-runner.rkt"

    if not racket_runner.exists():
        return BenchmarkResult(
            time_ms=0, conclusions=0, mode="standard",
            success=False, error="racket-runner.rkt not found"
        )

    # Check if racket is available
    if not shutil.which("racket"):
        return BenchmarkResult(
            time_ms=0, conclusions=0, mode="standard",
            success=False, error="racket not found in PATH"
        )

    cmd = ["racket", str(racket_runner), str(dfl_file)]

    try:
        start = time.perf_counter()
        result = subprocess.run(
            cmd, capture_output=True, text=True, timeout=300
        )
        elapsed_ms = (time.perf_counter() - start) * 1000

        if result.returncode != 0:
            return BenchmarkResult(
                time_ms=elapsed_ms, conclusions=0,
                mode="standard",
                success=False, error=result.stderr[:500]
            )

        # Parse JSON output
        try:
            data = json.loads(result.stdout.strip())
            return BenchmarkResult(
                time_ms=data["time_ms"],
                conclusions=data["conclusions"],
                mode=data["mode"],
                success=True
            )
        except json.JSONDecodeError:
            return BenchmarkResult(
                time_ms=elapsed_ms, conclusions=0,
                mode="standard",
                success=False, error=f"Invalid JSON output: {result.stdout[:200]}"
            )
    except subprocess.TimeoutExpired:
        return BenchmarkResult(
            time_ms=300000, conclusions=0,
            mode="standard",
            success=False, error="Timeout (5 min)"
        )
    except Exception as e:
        return BenchmarkResult(
            time_ms=0, conclusions=0,
            mode="standard",
            success=False, error=str(e)
        )


def percentile(data: list[float], p: int) -> float:
    """Calculate the p-th percentile of data."""
    if not data:
        return 0.0
    sorted_data = sorted(data)
    idx = min(len(sorted_data) - 1, int((p / 100) * len(sorted_data)))
    return sorted_data[idx]


def mean(data: list[float]) -> float:
    """Calculate the mean of data."""
    return sum(data) / len(data) if data else 0.0


def run_tier_benchmark(
    tier: TierConfig,
    iterations: int = 5,
    verbose: bool = True
) -> dict:
    """Run a benchmark tier and return results."""

    # Generate theory based on tier type
    if tier.name == "Chain":
        theory_dfl = generate_chain_theory_dfl(tier.rules)
    elif tier.name == "Conflict":
        theory_dfl = generate_conflict_heavy_theory_dfl(
            num_chains=tier.facts,
            chain_depth=tier.rules // tier.facts
        )
    else:
        theory_dfl = generate_theory_dfl(tier.facts, tier.rules)

    # Write to temp file
    with tempfile.NamedTemporaryFile(mode='w', suffix='.dfl', delete=False) as f:
        f.write(theory_dfl)
        dfl_file = Path(f.name)

    try:
        rust_times = []
        racket_times = []
        rust_conclusions = 0
        racket_conclusions = 0
        rust_error = None
        racket_error = None

        mode_str = "standard"

        for i in range(iterations):
            if verbose:
                print(f"  Iteration {i + 1}/{iterations}...", end="", flush=True)

            # Run Rust
            rust_result = run_rust_benchmark(dfl_file)
            if rust_result.success:
                rust_times.append(rust_result.time_ms)
                rust_conclusions = rust_result.conclusions
            else:
                rust_error = rust_result.error

            # Run Racket
            racket_result = run_racket_benchmark(dfl_file)
            if racket_result.success:
                racket_times.append(racket_result.time_ms)
                racket_conclusions = racket_result.conclusions
            else:
                racket_error = racket_result.error

            if verbose:
                rust_ms = f"{rust_result.time_ms:.1f}ms" if rust_result.success else "ERR"
                racket_ms = f"{racket_result.time_ms:.1f}ms" if racket_result.success else "ERR"
                print(f" Rust: {rust_ms}, Racket: {racket_ms}")

        return {
            "tier": tier.name,
            "rules": tier.rules,
            "facts": tier.facts,
            "mode": mode_str,
            "iterations": iterations,
            "rust": {
                "times": rust_times,
                "p95": percentile(rust_times, 95) if rust_times else None,
                "mean": mean(rust_times) if rust_times else None,
                "min": min(rust_times) if rust_times else None,
                "max": max(rust_times) if rust_times else None,
                "conclusions": rust_conclusions,
                "error": rust_error,
            },
            "racket": {
                "times": racket_times,
                "p95": percentile(racket_times, 95) if racket_times else None,
                "mean": mean(racket_times) if racket_times else None,
                "min": min(racket_times) if racket_times else None,
                "max": max(racket_times) if racket_times else None,
                "conclusions": racket_conclusions,
                "error": racket_error,
            },
            "target_ms": tier.target_ms,
        }
    finally:
        dfl_file.unlink(missing_ok=True)


def print_results(results: list[dict]):
    """Print benchmark results in a formatted table."""
    print("\n" + "=" * 80)
    print("BENCHMARK RESULTS")
    print("=" * 80)

    # Header
    print(f"\n{'Tier':<12} {'Mode':<10} {'Rust P95':<12} {'Racket P95':<12} {'Speedup':<10} {'Target':<10}")
    print("-" * 76)

    for r in results:
        tier = r["tier"]
        mode = r["mode"]

        rust_p95 = r["rust"]["p95"]
        racket_p95 = r["racket"]["p95"]
        target = r["target_ms"]

        rust_str = f"{rust_p95:.1f}ms" if rust_p95 else "ERROR"
        racket_str = f"{racket_p95:.1f}ms" if racket_p95 else "ERROR"

        if rust_p95 and racket_p95:
            speedup = racket_p95 / rust_p95
            speedup_str = f"{speedup:.2f}x"
        else:
            speedup_str = "N/A"

        target_str = f"<={target}ms"

        print(f"{tier:<12} {mode:<10} {rust_str:<12} {racket_str:<12} {speedup_str:<10} {target_str:<10}")

    # Summary
    print("\n" + "-" * 76)
    print("Notes:")
    print("  - Speedup > 1.0 means Rust is faster than Racket")
    print("  - P95 = 95th percentile execution time")
    print("  - Target from NFR benchmark specifications")

    # Check conclusion matching
    for r in results:
        rust_conc = r["rust"]["conclusions"]
        racket_conc = r["racket"]["conclusions"]
        if rust_conc and racket_conc and rust_conc != racket_conc:
            print(f"\n  WARNING: Conclusion mismatch in {r['tier']} ({r['mode']}): "
                  f"Rust={rust_conc}, Racket={racket_conc}")

    # Print any errors
    errors = []
    for r in results:
        if r["rust"]["error"]:
            errors.append(f"  Rust ({r['tier']}): {r['rust']['error']}")
        if r["racket"]["error"]:
            errors.append(f"  Racket ({r['tier']}): {r['racket']['error']}")

    if errors:
        print("\nErrors:")
        for e in errors:
            print(e)


def main():
    parser = argparse.ArgumentParser(
        description="Benchmark spindle-rust vs spindle-racket"
    )
    parser.add_argument(
        "--tiers", "-t",
        default="small,medium,chain",
        help="Comma-separated list of tiers to benchmark (small,medium,large,chain,conflict)"
    )
    parser.add_argument(
        "--iterations", "-n",
        type=int, default=5,
        help="Number of iterations per benchmark"
    )
    parser.add_argument(
        "--json", "-j",
        action="store_true",
        help="Output results as JSON"
    )
    parser.add_argument(
        "--quiet", "-q",
        action="store_true",
        help="Suppress progress output"
    )

    args = parser.parse_args()

    # Parse tiers
    tier_names = [t.strip().lower() for t in args.tiers.split(",")]
    selected_tiers = []
    for name in tier_names:
        if name in TIERS:
            selected_tiers.append(TIERS[name])
        else:
            print(f"Unknown tier: {name}")
            print(f"Available tiers: {', '.join(TIERS.keys())}")
            sys.exit(1)

    if not args.quiet:
        print("=" * 60)
        print("Spindle Benchmark: Rust vs Racket")
        print("=" * 60)
        print(f"Tiers: {', '.join(t.name for t in selected_tiers)}")
        print(f"Iterations: {args.iterations}")
        print("Modes: standard")
        print()

    results = []

    for tier in selected_tiers:
        if not args.quiet:
            print(f"\n[{tier.name}] {tier.rules} rules, {tier.facts} facts (standard mode)")

        result = run_tier_benchmark(
            tier,
            iterations=args.iterations,
            verbose=not args.quiet
        )
        results.append(result)

    if args.json:
        print(json.dumps(results, indent=2))
    else:
        print_results(results)


if __name__ == "__main__":
    main()
