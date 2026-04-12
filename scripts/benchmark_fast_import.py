#!/usr/bin/env python3
import argparse
import json
import os
import statistics
import sys
import time
import urllib.error
import urllib.request


def parse_provider_list(raw: str) -> list[str]:
    providers = [item.strip() for item in raw.split(",") if item.strip()]
    if not providers:
        raise ValueError("at least one provider is required")
    return providers


def build_accounts(count: int, providers: list[str]) -> list[dict]:
    provider_count = len(providers)
    return [
        {
            "name": f"bench-{providers[i % provider_count]}-{i}",
            "provider": providers[i % provider_count],
            "credential_type": "api_key",
            "credential": f"sk-bench-{providers[i % provider_count]}-{i:08d}",
            "priority": 50,
            "concurrent_limit": 5,
        }
        for i in range(count)
    ]


def provider_mix(count: int, providers: list[str]) -> dict[str, int]:
    mix = {provider: 0 for provider in providers}
    for i in range(count):
        mix[providers[i % len(providers)]] += 1
    return mix


def submit_request(
    *,
    base_url: str,
    bearer: str,
    payload: dict,
    timeout_seconds: int,
) -> tuple[int, dict, float]:
    body = json.dumps(payload).encode("utf-8")
    request = urllib.request.Request(
        f"{base_url}/api/v1/admin/accounts/fast-import",
        data=body,
        method="POST",
        headers={
            "Authorization": f"Bearer {bearer}",
            "Content-Type": "application/json",
        },
    )

    started = time.perf_counter()
    try:
        with urllib.request.urlopen(request, timeout=timeout_seconds) as response:
            raw = response.read()
            status = response.status
    except urllib.error.HTTPError as exc:
        print(f"http error: {exc.code}", file=sys.stderr)
        print(exc.read().decode("utf-8", errors="replace"), file=sys.stderr)
        raise SystemExit(1)
    except Exception as exc:
        print(f"request failed: {exc}", file=sys.stderr)
        raise SystemExit(1)

    wall_ms = (time.perf_counter() - started) * 1000.0
    return status, json.loads(raw), wall_ms


def normalize_run(
    *,
    status: int,
    data: dict,
    wall_ms: float,
    count: int,
    providers: list[str],
    dry_run: bool,
    fast_mode: bool,
    iteration: int,
) -> dict:
    section = data.get("preview") if dry_run else data
    return {
        "iteration": iteration,
        "status": status,
        "count": count,
        "providers_requested": providers,
        "provider_mix": provider_mix(count, providers),
        "dry_run": dry_run,
        "fast_mode": fast_mode,
        "duration_ms": section.get("duration_ms"),
        "throughput_items_per_sec": section.get("throughput_items_per_sec"),
        "wall_clock_duration_ms": data.get("wall_clock_duration_ms", round(wall_ms, 2)),
        "imported": section.get("imported"),
        "will_import": section.get("will_import"),
        "failed": section.get("failed", section.get("invalid")),
        "providers": section.get("providers", []),
    }


def summarize_runs(runs: list[dict]) -> dict:
    throughputs = [run["throughput_items_per_sec"] for run in runs if isinstance(run.get("throughput_items_per_sec"), (int, float))]
    durations = [run["duration_ms"] for run in runs if isinstance(run.get("duration_ms"), (int, float))]
    wall_durations = [
        run["wall_clock_duration_ms"] for run in runs if isinstance(run.get("wall_clock_duration_ms"), (int, float))
    ]

    return {
        "runs": len(runs),
        "throughput_avg": round(statistics.mean(throughputs), 2) if throughputs else None,
        "throughput_p95": round(max(throughputs), 2) if throughputs else None,
        "duration_avg_ms": round(statistics.mean(durations), 2) if durations else None,
        "wall_clock_avg_ms": round(statistics.mean(wall_durations), 2) if wall_durations else None,
        "best_run": max(runs, key=lambda item: item.get("throughput_items_per_sec") or -1) if runs else None,
        "worst_run": min(runs, key=lambda item: item.get("throughput_items_per_sec") or 10**18) if runs else None,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Benchmark FoxNIO fast import endpoint")
    parser.add_argument("--count", type=int, default=1000, help="Number of accounts to generate")
    parser.add_argument(
        "--provider",
        default="openai",
        help="Single provider label to stamp into generated accounts",
    )
    parser.add_argument(
        "--providers",
        help="Comma-separated provider mix, e.g. openai,anthropic,gemini",
    )
    parser.add_argument("--batch-size", type=int, default=1000)
    parser.add_argument("--validation-concurrency", type=int, default=50)
    parser.add_argument("--dry-run", action="store_true", help="Only run preview mode")
    parser.add_argument("--fast-mode", action="store_true", help="Skip validation in service")
    parser.add_argument("--repeat", type=int, default=1, help="Repeat the same benchmark N times")
    parser.add_argument("--timeout", type=int, default=300, help="HTTP timeout in seconds")
    parser.add_argument(
        "--format",
        choices=("json", "jsonl", "markdown"),
        default="json",
        help="Output format",
    )
    args = parser.parse_args()

    base_url = os.environ.get("FOXNIO_BASE_URL", "http://localhost:8080").rstrip("/")
    bearer = os.environ.get("FOXNIO_ADMIN_BEARER")
    if not bearer:
        print("missing FOXNIO_ADMIN_BEARER", file=sys.stderr)
        return 2

    providers = parse_provider_list(args.providers) if args.providers else [args.provider]
    payload = {
        "accounts": build_accounts(args.count, providers),
        "batch_size": args.batch_size,
        "validation_concurrency": args.validation_concurrency,
        "skip_duplicates": True,
        "fast_mode": args.fast_mode,
        "dry_run": args.dry_run,
    }

    runs = []
    for iteration in range(1, args.repeat + 1):
        status, data, wall_ms = submit_request(
            base_url=base_url,
            bearer=bearer,
            payload=payload,
            timeout_seconds=args.timeout,
        )
        runs.append(
            normalize_run(
                status=status,
                data=data,
                wall_ms=wall_ms,
                count=args.count,
                providers=providers,
                dry_run=args.dry_run,
                fast_mode=args.fast_mode,
                iteration=iteration,
            )
        )

    result = {
        "scenario": {
            "count": args.count,
            "providers_requested": providers,
            "batch_size": args.batch_size,
            "validation_concurrency": args.validation_concurrency,
            "dry_run": args.dry_run,
            "fast_mode": args.fast_mode,
            "repeat": args.repeat,
        },
        "summary": summarize_runs(runs),
        "runs": runs,
    }

    if args.format == "jsonl":
        for run in runs:
            print(json.dumps(run, ensure_ascii=False))
    elif args.format == "markdown":
        print("# FoxNIO fast-import benchmark")
        print()
        print(
            f"- count: `{args.count}`  \n- providers: `{', '.join(providers)}`  \n- dry_run: `{args.dry_run}`  \n- fast_mode: `{args.fast_mode}`  \n- repeat: `{args.repeat}`"
        )
        print()
        print("| run | throughput items/s | duration ms | wall ms | imported | failed |")
        print("| --- | ---: | ---: | ---: | ---: | ---: |")
        for run in runs:
            print(
                f"| {run['iteration']} | {run.get('throughput_items_per_sec', '-')}"
                f" | {run.get('duration_ms', '-')}"
                f" | {run.get('wall_clock_duration_ms', '-')}"
                f" | {run.get('imported', run.get('will_import', '-'))}"
                f" | {run.get('failed', '-')}"
                " |"
            )
        print()
        print("## Summary")
        print()
        print(json.dumps(result["summary"], ensure_ascii=False, indent=2))
    else:
        print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
