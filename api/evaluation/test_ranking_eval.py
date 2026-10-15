import copy
from pathlib import Path
import tempfile
import unittest

from coverage_eval import EvaluationError
from ranking_eval import prepare, score_results


class RankingIntegrity(unittest.TestCase):
    def test_labels_never_enter_queries_and_holdout_is_rejected(self):
        case = {'id': 'c', 'split': 'development', 'input': {'statement': 'For every group...'},
                'label': {'argument': 'SECRET LABEL'}}
        with tempfile.TemporaryDirectory() as folder:
            queries, inputs, missing = prepare([case], Path(folder))
            self.assertEqual(queries, [{'id': 'c:raw', 'query': 'Question\nFor every group...'}])
            self.assertEqual(inputs, {})
            self.assertEqual(missing, ['c'])
            case['split'] = 'holdout'
            with self.assertRaises(EvaluationError):
                prepare([case], Path(folder))

    def test_changed_queries_missing_results_and_mixed_indexes_are_rejected(self):
        cases = [{'id': 'c', 'family': 'f', 'label': {'sources': []}}]
        queries = [{'id': 'c:raw', 'query': 'exact text'}]
        result = {'diagnostic_only': True, 'results': [{
            'id': 'c:raw', 'query': 'exact text', 'query_ms': 1,
            'result': {'snapshot': 's', 'index_revision': 'i', 'hits': []}}]}
        scores = score_results(cases, {}, queries, result, 's', 'i')
        self.assertEqual(scores['raw']['completed']['numerator'], 1)
        self.assertEqual(scores['terms']['completed']['denominator'], 1)
        self.assertEqual(scores['terms']['completed']['numerator'], 0)
        for mutation in ['query', 'missing', 'duplicate', 'index']:
            bad = copy.deepcopy(result)
            if mutation == 'query':
                bad['results'][0]['query'] = 'different input'
            elif mutation == 'missing':
                bad['results'] = []
            elif mutation == 'duplicate':
                bad['results'] *= 2
            else:
                bad['results'][0]['result']['index_revision'] = 'different index'
            with self.assertRaises(EvaluationError):
                score_results(cases, {}, queries, bad, 's', 'i')
