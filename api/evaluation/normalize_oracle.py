#!/usr/bin/env python3
"""Normalize retained statement-only Oracle answers without assigning audit status.

The input directory contains metadata.json and per-case answer.txt/receipt.json
from acquisition. Incomplete/failed transport is excluded from observations and
explicitly listed; the scorer retains it in the corpus denominator.
"""

import argparse
import json
from pathlib import Path

from coverage_eval import corpus, digest, read, require, score


def last_object(text):
    # Same last-complete-object rule as the application's referee parser: browser
    # extraction can remove fences while mathematical prose contains braces.
    decoder, found, at = json.JSONDecoder(), None, 0
    while (start := text.find("{", at)) >= 0:
        try:
            value, length = decoder.raw_decode(text[start:])
        except ValueError:
            at = start + 1
            continue
        if isinstance(value, dict):
            found = value
        at = start + length
    return found


def normalize(root, data, freeze):
    metadata = read(root / "metadata.json")
    require(metadata["freeze_sha256"] == freeze, "acquisition uses another corpus")
    predictions, evidence, failures, models = [], [], [], set()
    for case in data["cases"]:
        if case["split"] != "development":
            continue
        directory = root / case["id"]
        receipt_file = directory / "receipt.json"
        if not receipt_file.exists():
            failures.append({"id": case["id"], "reason": "acquisition_pending"})
            continue
        receipt = read(receipt_file)
        require(receipt["case_id"] == case["id"], "receipt case mismatch")
        require(digest(directory / "response.sse") == receipt["response_sha256"], "stream changed")
        require(digest(directory / "request.json") == receipt["request_sha256"], "request changed")
        if receipt["exit_code"] != 0 or not receipt["stream_done"] or receipt["errors"]:
            failures.append({"id": case["id"], "reason": "transport_incomplete",
                             "receipt_sha256": digest(receipt_file)})
            continue
        answer = (directory / "answer.txt").read_text()
        # Verify that the extracted answer is exactly the saved provider stream.
        streamed = ""
        for line in (directory / "response.sse").read_text().splitlines():
            if line.startswith("data: ") and line != "data: [DONE]":
                chunk = json.loads(line[6:])
                streamed += "".join((c.get("delta") or {}).get("content") or ""
                                    for c in chunk.get("choices", []))
        require(answer == streamed, "extracted answer differs from provider stream")
        report = last_object(answer)
        claims = report.get("claims", []) if report else []
        if len(claims) != 1 or claims[0].get("claim") != "C1":
            failures.append({"id": case["id"], "reason": "unusable_report"})
            continue
        reading = claims[0].get("conjecture") or {}
        status = reading.get("status")
        relation = {"known_true": "covered", "special_case_of_known": "covered",
                    "known_false": "refuted", "open": "uncovered", "unclear": "unknown"}.get(status)
        if relation is None:
            failures.append({"id": case["id"], "reason": "unusable_status"})
            continue
        observed = receipt["oracle"]
        model = (observed.get("observed_model_switcher"), observed.get("observed_model_effort"))
        models.add(model)
        predictions.append({"id": case["id"], "relation": relation,
                            "query_status": "not_configured", "returned_sources": [],
                            "novelty_claim": status == "open" and reading.get("escape") == "content",
                            "latency_ms": receipt["elapsed_ms"], "cost_usd": None})
        evidence.append({"id": case["id"], "provider_task_id": observed.get("task_id"),
                         "observed_model": model, "reported_status": status,
                         "answer_sha256": digest(directory / "answer.txt"),
                         "stream_sha256": receipt["response_sha256"],
                         "request_sha256": receipt["request_sha256"],
                         "qualification": "Oracle statement reading; no independent source audit"})
    run = {"schema": "mathesis.coverage-evaluation-run.v1", "freeze_sha256": freeze,
           "split": "development", "implementation_revision": metadata["implementation_revision"],
           "method": "contract 3 statement-only Oracle referee; no CMA source audit",
           "model": sorted(models, key=str) or "no observed model",
           "snapshot": "not configured in the measured application revision",
           "budget": {"cases": 12, "parallel_requests": 2, "monetary_cost": "unmeasured"},
           "started_at_utc": metadata["started_at_utc"], "predictions": predictions,
           "normalization": "known_true/special_case_of_known→covered; known_false→refuted; open→uncovered; unclear→unknown; explicit open+content→novelty assertion",
           "evidence": evidence, "acquisition_failures": failures}
    return run


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("acquisition", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    data, freeze = corpus(Path(__file__).parent / "coverage_v1")
    run = normalize(args.acquisition, data, freeze)
    # Explicit immutable outputs; do not overwrite an earlier measured result.
    with args.output.open("x") as target:
        json.dump(run, target, indent=2, ensure_ascii=False)
        target.write("\n")
    result = score(data, freeze, run, "development")
    result["run_sha256"] = digest(args.output)
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
