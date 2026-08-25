# Resource and Wasm Benchmarks

This document records the resource baselines enforced by CI. The committed source of truth is [`benchmarks/budget-baselines.env`](../benchmarks/budget-baselines.env); changing a budget therefore requires an explicit reviewed diff rather than a CI-only override.

## Measurement environment

- Repository base: `a87d6fa9eb8dc1d28e37184b168c5ab0b89257a3`
- Measured: 2026-08-25
- Rust: `1.98.0`
- Soroban SDK: `27.0.4`
- `soroban-budget-assert`: `c2433dc6eab003a2b815c22cb943b9c11d7e4c62`
- `soroban-cost-linter`: `6b595a867533822fbe8b15231e669574ffd5dcbd`
- Cost-linter toolchain: `nightly-2026-04-16`, Dylint `6.0.1`

The runtime tests execute in the SDK 27 test environment. Immediately before the operation under test, the SDK budget is reset so fixture creation, contract initialization, proof construction, and prefunding are excluded. CPU and memory are asserted with Tollcraft's `budget-macros`; read/write entry and byte counts come from `Env::cost_estimate().resources()`.

`read entries` is `memory_read_entries + disk_read_entries`. `read bytes` is `disk_read_bytes`. A zero disk-read-byte value is expected when the entry is already resident in the test environment; it remains a useful regression guard, but it is not a substitute for an RPC simulation against live ledger state.

Runtime ceilings are `ceil(baseline × 1.15)` (15% tolerance); a zero baseline keeps a zero ceiling. Wasm ceilings are `ceil(baseline × 1.10)` (10% tolerance).

## Current network limits

The comparison below uses the current Protocol 27 per-transaction limits published by Stellar: 400,000,000 CPU instructions, 40 MiB memory, 400 footprint entries, 200 disk-read entries / 200,000 disk-read bytes, 200 written entries / 132,096 written bytes, and 131,072 bytes per contract Wasm. These settings are validator-controlled and may change; verify them with `stellar network settings --network mainnet` when refreshing this report.

Reference: https://developers.stellar.org/docs/build/guides/storage/storage-strategies#appendix-network-limits

## Wasm size

| Contract | Baseline | CI ceiling (+10%) | Network max | Network headroom |
| --- | ---: | ---: | ---: | ---: |
| ReceiptAnchor | 18,226 B | 20,049 B | 131,072 B | 86.09% |
| RefundVault | 19,277 B | 21,205 B | 131,072 B | 85.29% |

## Runtime resources

`Min network headroom` is the smallest remaining percentage among CPU, memory, conservative footprint entries (`reads + writes`), disk-read bytes, written entries, and written bytes. Adding reads and writes is conservative because the same ledger key may appear in both sets.

| Scenario | CPU | Memory | Read entries | Read bytes | Write entries | Write bytes | Min network headroom |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `ReceiptAnchor::anchor_batch(count=1)` | 101,573 | 13,296 B | 5 | 0 B | 3 | 616 B | 98.00% |
| `ReceiptAnchor::anchor_batch(count=500)` | 101,573 | 13,296 B | 5 | 0 B | 3 | 616 B | 98.00% |
| `ReceiptAnchor::anchor_batch(count=1000)` | 101,573 | 13,296 B | 5 | 0 B | 3 | 616 B | 98.00% |
| `ReceiptAnchor::verify_receipt(depth=0)` | 34,058 | 4,246 B | 2 | 0 B | 0 | 0 B | 99.50% |
| `ReceiptAnchor::verify_receipt(depth=5)` | 83,753 | 5,686 B | 2 | 0 B | 0 | 0 B | 99.50% |
| `ReceiptAnchor::verify_receipt(depth=10)` | 133,448 | 7,126 B | 2 | 0 B | 0 | 0 B | 99.50% |
| `ReceiptAnchor::prune_batches(64 deletions)` | 4,364,230 | 848,093 B | 68 | 0 B | 66 | 336 B | 66.50% |
| `RefundVault::deposit` | 225,789 | 29,850 B | 8 | 92 B | 3 | 520 B | 97.25% |
| `RefundVault::refund` | 317,008 | 44,314 B | 9 | 92 B | 4 | 776 B | 96.75% |

CI also checks the corresponding 15%-headroom ceilings stored next to every baseline in `benchmarks/budget-baselines.env`. Any scenario above its committed ceiling fails the `budget` job.

## Why `MAX_BATCH_SIZE = 1000` remains safe

`anchor_batch` commits one Merkle root plus batch metadata; it does not iterate over the receipts represented by that root. That is visible in the measurements: counts 1, 500, and 1000 have identical CPU, memory, and ledger I/O. Increasing `count` up to the existing validation limit therefore does not increase the on-chain anchoring work in the current implementation.

A 1,000-leaf binary Merkle tree needs at most 10 proof siblings (`ceil(log2(1000)) = 10`) for the covered proof shape. The depth-10 verification baseline is 133,448 CPU instructions and 7,126 bytes of memory, leaving more than 99% headroom against those network limits. The measurements therefore do not justify lowering `MAX_BATCH_SIZE`; it remains 1000.

`prune_batches` is different: its work scales with the number of stored batch records traversed. The 64-deletion benchmark intentionally exercises a sizable cleanup range and still keeps 66.5% conservative footprint headroom. The static Tollcraft linter is run in CI so loop/storage findings remain visible. If pruning behavior or network limits change, remeasure this scenario before increasing the benchmarked range.

## Updating a baseline deliberately

1. Make the contract change and run the `budget` job locally or in CI with the pinned tool versions above.
2. Confirm the resource increase is expected and still leaves appropriate network headroom.
3. Update the `*_BASELINE` value and recompute its committed ceiling (`+15%` runtime, `+10%` Wasm) in `benchmarks/budget-baselines.env`.
4. Update the matching row and rationale in this document in the same PR.
5. Do not bypass the budget job with environment variables or uncommitted CI configuration.
