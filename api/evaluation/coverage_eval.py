#!/usr/bin/env python3
"""Offline evaluation of recorded coverage observations; never calls a model.

Only the `inputs` command exports model inputs. Labels and copied source evidence
stay with the evaluator. A held-out run requires a preregistration tied to the
exact freeze and an already recorded development baseline.
"""

import argparse
import hashlib
import json
import math
from collections import Counter
from pathlib import Path


class EvaluationError(ValueError):
    pass


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read(path):
    def unique(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise EvaluationError(f"duplicate JSON key: {key}")
            result[key] = value
        return result

    return json.loads(Path(path).read_text(), object_pairs_hook=unique)


def require(condition, reason):
    if not condition:
        raise EvaluationError(reason)


def corpus(root):
    root = Path(root).resolve()
    freeze = read(root / "freeze.json")
    require(freeze.get("schema") == "mathesis.evaluation-freeze.v1", "unknown freeze")
    for name, expected in freeze["files"].items():
        path = (root / name).resolve()
        require(path.is_relative_to(root), "freeze path escapes corpus")
        require(digest(path) == expected, f"frozen file changed: {name}")
    require({"corpus.json", "sources.json"} <= freeze["files"].keys(), "unfrozen inputs")
    data, sources = read(root / "corpus.json"), read(root / "sources.json")
    require(data.get("schema") == "mathesis.coverage-evaluation.v1", "unknown corpus")
    ids, groups = set(), {}
    for case in data["cases"]:
        require(case["id"] not in ids, "duplicate case")
        ids.add(case["id"])
        require(case["split"] in {"development", "holdout"}, "invalid split")
        require(groups.setdefault(case["family"], case["split"]) == case["split"],
                "theorem family leaks across splits")
        require(case["label"]["relation"] in {"covered", "refuted", "not_established"},
                "unsupported truth label (global novelty is not a label)")
        require(bool(case["label"]["argument"].strip()), "missing label argument")
        for source in case["label"]["sources"]:
            known = sources[source["family"]]
            require(known["source_commit"] == data["source_snapshot"]["source_commit"],
                    "source revision differs from snapshot")
            require(any(d["statement_id"] == source["statement_id"]
                        and d["declaration_name_key"] == source["declaration_name_key"]
                        for d in known["declarations"]), "unresolved label declaration")
    require(groups == freeze["family_splits"], "family allocation changed")
    for family, source in sources.items():
        name = f"sources/{family}.lean"
        require(name in freeze["files"], "unfrozen source")
        require(source["source_sha256"] == "sha256:" + digest(root / name),
                "source bytes differ from inspector report identity")
    return data, digest(root / "freeze.json")


def preregistered(path, freeze_hash):
    require(path is not None, "holdout needs a preregistration file")
    path = Path(path)
    registration = read(path)
    require(registration.get("schema") == "mathesis.evaluation-preregistration.v1",
            "unknown preregistration")
    require(registration.get("freeze_sha256") == freeze_hash, "wrong preregistered corpus")
    require(bool(registration.get("recorded_at_utc")), "missing registration time")
    require(bool(registration.get("candidate_revision")), "missing candidate revision")
    require(bool(registration.get("targets")), "targets must be fixed before holdout")
    baseline = path.parent / registration["development_baseline_file"]
    require(digest(baseline) == registration["development_baseline_sha256"],
            "development baseline changed")
    observed = read(baseline)
    require(observed.get("split") == "development"
            and observed.get("freeze_sha256") == freeze_hash
            and observed.get("observations", 0) > 0, "missing measured development baseline")
    return registration


def ratio(numerator, denominator):
    # Undefined denominators are not perfect scores, failures, or zero cost.
    return {"numerator": numerator, "denominator": denominator,
            "value": numerator / denominator if denominator else None}


def score(data, freeze_hash, run, split):
    require(run.get("schema") == "mathesis.coverage-evaluation-run.v1", "unknown run")
    require(run.get("freeze_sha256") == freeze_hash, "run uses another corpus")
    require(run.get("split") == split, "run split differs")
    for key in ("implementation_revision", "method", "model", "snapshot", "budget", "started_at_utc"):
        require(bool(run.get(key)), f"run missing {key}")
    cases = {c["id"]: c for c in data["cases"] if c["split"] == split}
    predictions = {}
    for p in run["predictions"]:
        require(p["id"] in cases, "prediction outside selected split")
        require(p["id"] not in predictions, "duplicate prediction")
        require(p["relation"] in {"covered", "refuted", "candidate", "unknown", "uncovered"},
                "invalid observed relation")
        require(p["query_status"] in {"complete", "partial", "unavailable", "timeout",
                                      "invalid_artifact", "not_configured"}, "invalid query status")
        require(type(p.get("novelty_claim")) is bool, "novelty claim must be explicit")
        require(isinstance(p.get("returned_sources"), list), "missing retrieval observations")
        for ref in p["returned_sources"]:
            require(isinstance(ref, dict) and all(isinstance(ref.get(k), str) and ref[k]
                    for k in ("family", "statement_id", "declaration_name_key")),
                    "retrieval identity must retain module and declaration")
        for field in ("latency_ms", "cost_usd"):
            value = p.get(field)
            require(value is None or (type(value) in (int, float) and math.isfinite(value)
                                     and value >= 0), "invalid measured resource value")
        predictions[p["id"]] = p
    # Partial acquisition remains visible. Missing predictions count in total
    # completion and positive recall denominators; they are never dropped.
    counts = Counter()
    states, domains = Counter(), {}
    latencies, costs = [], []
    for case_id, case in cases.items():
        p = predictions.get(case_id)
        label = case["label"]["relation"]
        domain = domains.setdefault(case["domain"], Counter())
        domain["cases"] += 1
        if label == "covered":
            counts["positive"] += 1
            domain["positive"] += 1
        if p is None:
            states["missing_observation"] += 1
            continue
        states[p["query_status"]] += 1
        counts["correct_relation"] += p["relation"] == label
        counts["abstained"] += p["relation"] in {"candidate", "unknown"}
        if label == "covered":
            found = p["relation"] == "covered"
            counts["detected"] += found
            domain["detected"] += found
            expected = {(s["family"], s["statement_id"], s["declaration_name_key"])
                        for s in case["label"]["sources"]}
            returned = {(s["family"], s["statement_id"], s["declaration_name_key"])
                        for s in p["returned_sources"]}
            counts["all_premises_retrieved"] += expected <= returned
            counts["known_reported_uncovered"] += p["relation"] == "uncovered"
        asserted = p["relation"] in {"covered", "refuted"}
        counts["asserted_relation"] += asserted
        counts["unsupported_relation"] += asserted and p["relation"] != label
        counts["novelty_assertions"] += p["novelty_claim"]
        counts["unsupported_novelty"] += p["novelty_claim"]  # corpus has no novelty truth
        counts["incomplete_novelty"] += p["novelty_claim"] and p["query_status"] != "complete"
        if p.get("latency_ms") is not None:
            latencies.append(p["latency_ms"])
        if p.get("cost_usd") is not None:
            costs.append(p["cost_usd"])
    return {
        "schema": "mathesis.coverage-evaluation-score.v1", "freeze_sha256": freeze_hash,
        "split": split, "cases": len(cases), "observations": len(predictions),
        "label_status": data["label_status"], "query_states": dict(states),
        "coverage_recall": ratio(counts["detected"], counts["positive"]),
        "complete_premise_retrieval": ratio(counts["all_premises_retrieved"], counts["positive"]),
        "known_reported_uncovered": ratio(counts["known_reported_uncovered"], counts["positive"]),
        "unsupported_relation": ratio(counts["unsupported_relation"], counts["asserted_relation"]),
        "abstention": ratio(counts["abstained"], len(predictions)),
        "unsupported_novelty_assertions": counts["unsupported_novelty"],
        "incomplete_novelty_assertions": counts["incomplete_novelty"],
        "per_domain": {name: {"cases": c["cases"], "coverage_recall":
                       ratio(c["detected"], c["positive"])} for name, c in domains.items()},
        "latency_ms": {"observed": len(latencies), "mean":
                       sum(latencies) / len(latencies) if latencies else None},
        "cost_usd": {"observed": len(costs), "partial_total": sum(costs) if costs else None},
        "limits": ["Authored evaluation cases; source-based labels are not human adjudication.",
                   "A returned identifier measures retrieval, not entailment or certification.",
                   "No observed output is treated as a system query receipt or Lean certificate.",
                   "No global novelty estimate; results apply only to this finite corpus."],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["validate", "inputs", "score"])
    parser.add_argument("--corpus", type=Path, default=Path(__file__).parent / "coverage_v1")
    parser.add_argument("--split", choices=["development", "holdout"], default="development")
    parser.add_argument("--preregistration", type=Path)
    parser.add_argument("--run", type=Path)
    args = parser.parse_args()
    data, freeze_hash = corpus(args.corpus)
    if args.command != "validate" and args.split == "holdout":
        preregistered(args.preregistration, freeze_hash)
    if args.command == "validate":
        result = {"freeze_sha256": freeze_hash, "cases": len(data["cases"]),
                  "splits": dict(Counter(c["split"] for c in data["cases"]))}
    elif args.command == "inputs":
        result = {"freeze_sha256": freeze_hash, "split": args.split,
                  "cases": [{"id": c["id"], **c["input"]} for c in data["cases"]
                            if c["split"] == args.split]}
    else:
        require(args.run is not None, "score requires --run")
        result = score(data, freeze_hash, read(args.run), args.split)
        result["run_sha256"] = digest(args.run)
    print(json.dumps(result, indent=2, ensure_ascii=False))


if __name__ == "__main__":
    main()
