#!/usr/bin/env python3
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
BASELINES = ROOT / "benchmarks" / "budget-baselines.env"


def load_values() -> dict[str, int]:
    values: dict[str, int] = {}
    for raw in BASELINES.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        key, value = line.split("=", 1)
        values[key.strip()] = int(value.strip())
    return values


def main() -> int:
    values = load_values()
    checks = (
        (
            "receipt-anchor",
            ROOT / "target/wasm32v1-none/release/receipt_anchor.wasm",
            "RECEIPT_WASM_BYTES_BASELINE",
            "RECEIPT_WASM_BYTES_LIMIT",
        ),
        (
            "refund-vault",
            ROOT / "target/wasm32v1-none/release/refund_vault.wasm",
            "REFUND_WASM_BYTES_BASELINE",
            "REFUND_WASM_BYTES_LIMIT",
        ),
    )

    failed = False
    for name, wasm, baseline_key, limit_key in checks:
        if not wasm.is_file():
            print(f"error: missing Wasm artifact: {wasm}", file=sys.stderr)
            failed = True
            continue
        actual = wasm.stat().st_size
        baseline = values[baseline_key]
        limit = values[limit_key]
        delta = ((actual - baseline) / baseline * 100.0) if baseline else 0.0
        status = "PASS" if actual <= limit else "FAIL"
        print(
            f"{status} {name}: actual={actual} B baseline={baseline} B "
            f"limit={limit} B delta={delta:+.2f}%"
        )
        if actual > limit:
            failed = True

    if failed:
        print(
            "Wasm size budget exceeded. Optimize the contract or deliberately "
            "remeasure and update benchmarks/budget-baselines.env plus docs/BENCHMARKS.md.",
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
