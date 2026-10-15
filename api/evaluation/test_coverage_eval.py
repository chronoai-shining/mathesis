"""Scorer regressions and exact, independently replayed finite label evidence."""

import copy
import itertools
import json
import tempfile
import unittest
from fractions import Fraction as Q
from pathlib import Path

import coverage_eval as ev


ROOT = Path(__file__).parent / "coverage_v1"


def multiply(a, b):
    return [[sum(x * y for x, y in zip(row, col)) for col in zip(*b)] for row in a]


class EvaluationTests(unittest.TestCase):
    def setUp(self):
        self.data, self.freeze = ev.corpus(ROOT)
        self.run = {
            "schema": "mathesis.coverage-evaluation-run.v1", "freeze_sha256": self.freeze,
            "split": "development", "implementation_revision": "synthetic-test-only",
            "method": "scorer unit test", "model": "none", "snapshot": "fixture",
            "budget": {"requests": 0}, "started_at_utc": "2026-10-16T00:00:00Z",
            "predictions": [],
        }

    def prediction(self, case_id, relation="covered", status="complete"):
        case = next(c for c in self.data["cases"] if c["id"] == case_id)
        return {"id": case_id, "relation": relation, "query_status": status,
                "novelty_claim": False, "returned_sources": case["label"]["sources"],
                "latency_ms": None, "cost_usd": None}

    def score(self):
        return ev.score(self.data, self.freeze, self.run, "development")

    def test_missing_predictions_remain_in_denominators(self):
        score = self.score()
        self.assertEqual(score["cases"], 12)
        self.assertEqual(score["coverage_recall"]["denominator"], 6)
        self.assertEqual(score["query_states"], {"missing_observation": 12})
        self.assertIsNone(score["unsupported_relation"]["value"])
        self.assertIsNone(score["cost_usd"]["partial_total"])

    def test_partial_query_can_still_establish_a_specific_relation(self):
        self.run["predictions"] = [self.prediction("pigeonhole-renamed", status="partial")]
        score = self.score()
        self.assertEqual(score["coverage_recall"]["numerator"], 1)
        self.assertEqual(score["unsupported_relation"]["numerator"], 0)
        self.assertEqual(score["query_states"]["partial"], 1)

    def test_joint_premises_are_not_independent_entailment_edges(self):
        p = self.prediction("bonferroni-bracket")
        p["returned_sources"] = p["returned_sources"][:1]
        self.run["predictions"] = [p]
        self.assertEqual(self.score()["complete_premise_retrieval"]["numerator"], 0)

    def test_wrong_declaration_identity_does_not_count_as_retrieval(self):
        p = self.prediction("cassini-renamed")
        p["returned_sources"] = copy.deepcopy(p["returned_sources"])
        p["returned_sources"][0]["declaration_name_key"] = "different declaration"
        self.run["predictions"] = [p]
        self.assertEqual(self.score()["complete_premise_retrieval"]["numerator"], 0)

    def test_missing_hypothesis_and_incomplete_novelty_are_distinct_failures(self):
        p = self.prediction("cassini-roots_dropped", status="timeout")
        p["novelty_claim"] = True
        self.run["predictions"] = [p]
        score = self.score()
        self.assertEqual(score["unsupported_relation"]["value"], 1)
        self.assertEqual(score["incomplete_novelty_assertions"], 1)

    def test_holdout_cannot_be_smuggled_into_development(self):
        self.run["predictions"] = [self.prediction("pure_state-renamed")]
        with self.assertRaisesRegex(ev.EvaluationError, "outside"):
            self.score()

    def test_duplicate_rows_and_non_finite_resource_values_rejected(self):
        p = self.prediction("pigeonhole-renamed")
        self.run["predictions"] = [p, p]
        with self.assertRaisesRegex(ev.EvaluationError, "duplicate prediction"):
            self.score()
        self.run["predictions"] = [p]
        p["cost_usd"] = float("nan")
        with self.assertRaisesRegex(ev.EvaluationError, "resource"):
            self.score()

    def test_holdout_needs_measured_baseline_and_fixed_thresholds(self):
        with self.assertRaisesRegex(ev.EvaluationError, "preregistration"):
            ev.preregistered(None, self.freeze)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            baseline = root / "baseline.json"
            baseline.write_text(json.dumps(self.score()))
            registration = root / "registration.json"
            registration.write_text(json.dumps({
                "schema": "mathesis.evaluation-preregistration.v1",
                "freeze_sha256": self.freeze, "recorded_at_utc": "2026-10-16T00:00:00Z",
                "candidate_revision": "synthetic", "targets": {"false_entailment": 0},
                "development_baseline_file": "baseline.json",
                "development_baseline_sha256": ev.digest(baseline),
            }))
            with self.assertRaisesRegex(ev.EvaluationError, "measured development"):
                ev.preregistered(registration, self.freeze)

    def test_freeze_detects_source_changes(self):
        import shutil
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "corpus"
            shutil.copytree(ROOT, root)
            with (root / "sources/pigeonhole.lean").open("a") as source:
                source.write("\n-- changed\n")
            with self.assertRaisesRegex(ev.EvaluationError, "frozen file changed"):
                ev.corpus(root)

    def test_label_evidence_is_never_claimed_as_human_review(self):
        self.assertFalse(any(c["label"]["human_adjudicated"] for c in self.data["cases"]))


class ExactLabelChecks(unittest.TestCase):
    def test_singleton_identity_has_no_distinct_collision(self):
        self.assertFalse(any(x != y and x == y for x in [0] for y in [0]))

    def test_cassini_without_root_assumptions_is_false(self):
        u = lambda j: 0 ** j + 0 ** j
        self.assertEqual(u(2) ** 2 - u(2) * u(1) - u(1) ** 2, 0)
        self.assertEqual(-5 * (-1) ** 1, 5)

    def test_projection_counterexample_uses_exact_rationals(self):
        vectors = [[2 * (i == j) - 1 for j in range(3)] for i in range(3)]
        projections = [[[Q(a * b, 3) for b in v] for a in v] for v in vectors]
        for p in projections:
            self.assertEqual(multiply(p, p), p)
            self.assertEqual([list(r) for r in zip(*p)], p)
        mean = [[sum(p[i][j] for p in projections) / 3 for j in range(3)] for i in range(3)]
        cube = multiply(multiply(mean, mean), mean)
        products = [multiply(multiply(projections[a], projections[b]), projections[c])
                    for a, b, c in itertools.permutations(range(3))]
        quadratic = sum(cube[i][j] - sum(p[i][j] for p in products) / 6
                        for i in range(3) for j in range(3))
        self.assertEqual(quadratic, Q(-8, 243))

    def test_two_projection_identity_symbolically_preserves_word_order(self):
        # Noncommutative expansion: P²=P and Q²=Q are the only relations.
        def product(left, right):
            result = {}
            for a, ca in left.items():
                for b, cb in right.items():
                    word = a + b
                    while "PP" in word or "QQ" in word:
                        word = word.replace("PP", "P").replace("QQ", "Q")
                    result[word] = result.get(word, 0) + ca * cb
            return {w: c for w, c in result.items() if c}
        mean_square = product({"P": Q(1, 2), "Q": Q(1, 2)},
                              {"P": Q(1, 2), "Q": Q(1, 2)})
        for word in ["PQ", "QP"]:
            mean_square[word] -= Q(1, 2)
        self.assertEqual(mean_square,
                         product({"P": Q(1, 2), "Q": Q(-1, 2)},
                                 {"P": Q(1, 2), "Q": Q(-1, 2)}))

    def test_signed_capture_weight_reverses_the_bound(self):
        weight, count = (-1) ** (3 * 3), 3
        self.assertGreater(weight, count * weight)

    def test_normalization_is_needed_for_idempotence(self):
        rho = multiply([[2]], [[2]])
        self.assertEqual(rho, [[4]])
        self.assertEqual(multiply(rho, rho), [[16]])

    def test_turan_condition_cannot_be_removed(self):
        t, q0 = Q(-2), Q(1)
        q1 = t
        q2 = ((t - 1) * q1 - q0) / 2
        self.assertEqual(q1 * q1 - 2 * q0 * q2, -1)


if __name__ == "__main__":
    unittest.main()
