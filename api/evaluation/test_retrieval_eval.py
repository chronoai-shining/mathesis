import unittest

from coverage_eval import EvaluationError
from retrieval_eval import assess, validate_response


class RetrievalMetrics(unittest.TestCase):
    def test_joint_premises_need_both_exact_declarations(self):
        cases = [{'id': 'known', 'family': 'f', 'label': {'sources': [
            {'family': 'f', 'statement_id': 'a', 'declaration_name_key': 'A'},
            {'family': 'g', 'statement_id': 'b', 'declaration_name_key': 'B'},
        ]}}]
        sources = {'f': {'repo_path': 'F.lean'}, 'g': {'repo_path': 'G.lean'}}
        observed = [{'id': 'known', 'state': 'complete', 'latency_ms': 12}]
        def hit(module, statement, name):
            return {'module_path': module, 'declarations': [{'identity': {
                'statement_id': statement, 'name_key': name}}]}
        hits = [hit('F.lean', 'a', 'A'), hit('G.lean', 'b', 'wrong-name')]
        score = assess(cases, sources, observed, {'known': {'result': {'hits': hits}}})
        self.assertEqual(score['required_modules_at_6']['numerator'], 1)
        self.assertEqual(score['required_declarations_at_6']['numerator'], 0)
        hits[1] = hit('G.lean', 'b', 'B')
        score = assess(cases, sources, observed, {'known': {'result': {'hits': hits}}})
        self.assertEqual(score['required_declarations_at_6']['numerator'], 1)
        hits = [hits[0]] + [hit('Other.lean', 'x', 'X')] * 5 + [hits[1]]
        score = assess(cases, sources, observed, {'known': {'result': {'hits': hits}}})
        self.assertEqual(score['required_declarations_at_6']['numerator'], 0)
        self.assertEqual(score['required_declarations_at_10']['numerator'], 1)

    def test_failed_queries_remain_in_the_denominator(self):
        cases = [{'id': 'known', 'family': 'f', 'label': {'sources': [
            {'family': 'f', 'statement_id': 'a', 'declaration_name_key': 'A'}]}}]
        score = assess(cases, {'f': {'repo_path': 'F.lean'}},
                       [{'id': 'known', 'state': 'timeout', 'latency_ms': 180000}], {})
        self.assertEqual(score['completed'], {'numerator': 0, 'denominator': 1, 'value': 0})
        self.assertEqual(score['required_declarations_at_6']['denominator'], 1)
        with self.assertRaises(EvaluationError):
            assess(cases, {'f': {}}, [], {})

    def test_changed_index_and_fabricated_source_coordinates_are_rejected(self):
        response = {'diagnostic_only': True, 'result': {
            'snapshot': 's', 'index_revision': 'i', 'hits': []}}
        validate_response(response, 's', 'i')
        with self.assertRaises(EvaluationError):
            validate_response(response, 's', 'other-index')
        response['result']['hits'] = [{'module_path': 'F.lean', 'source_sha256': 'hash',
            'declarations': [{'identity': {'snapshot': 's', 'module_path': 'G.lean'},
                              'source_sha256': 'hash'}]}]
        with self.assertRaises(EvaluationError):
            validate_response(response, 's', 'i')


if __name__ == '__main__':
    unittest.main()

class OracleStreamIntegrity(unittest.TestCase):
    def test_incomplete_stream_retains_handle_and_never_claims_completion(self):
        from source_referee_eval import parse_stream
        import json
        chunk = {'choices': [{'delta': {'content': 'partial'}}], 'oracle': {'task_id': 'saved'}}
        text = 'data: ' + json.dumps(chunk) + '\n'
        answer, oracle, errors, done = parse_stream(text)
        self.assertEqual(answer, 'partial')
        self.assertEqual(oracle['task_id'], 'saved')
        self.assertEqual(errors, [])
        self.assertFalse(done)
        self.assertTrue(parse_stream(text + 'data: [DONE]\n')[3])
        self.assertTrue(parse_stream(text + 'data: invalid\n')[2])
