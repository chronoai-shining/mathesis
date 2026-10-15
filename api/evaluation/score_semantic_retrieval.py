#!/usr/bin/env python3
"""Score actual saved reformulation queries, separately from Oracle judgments."""
import argparse
import json
from pathlib import Path

from coverage_eval import digest, read, require
from normalize_oracle import last_object
from normalize_source_oracle import normalize_sources
from retrieval_eval import assess, validate_response
from semantic_retrieval_eval import query_text


def score_run(root, acquisition, initial, run):
    data, freeze, normalized = normalize_sources(acquisition, initial, root)
    manifest = read(run / 'manifest.json')
    first = read(initial / 'manifest.json')
    require(manifest['freeze_sha256'] == freeze, 'changed corpus')
    require(manifest['initial_retrieval_manifest_sha256'] == digest(initial / 'manifest.json'), 'changed initial queries')
    require(manifest['acquisition_metadata_sha256'] == digest(acquisition / 'metadata.json'), 'changed Oracle run')
    require(manifest['snapshot'] == first['snapshot'] and manifest['index_revision'] == first['index_revision'], 'changed source')
    ranker = manifest.get('ranking_protocol')
    require(ranker in {None, 'mathesis.source-ranker.identifier-paths.v1'}, 'unsupported ranker')
    available = {p['id'] for p in normalized['predictions']}
    cases = [c for c in data['cases'] if c['split'] == 'development']
    observations, responses = [], {}
    for case in cases:
        case_id = case['id']
        folder = run / case_id
        if not (folder / 'observation.json').exists():
            observations.append({'id': case_id, 'state': 'unavailable', 'latency_ms': None})
            continue
        require(case_id in available, 'query has no usable saved Oracle proposal')
        observation, request = read(folder / 'observation.json'), read(folder / 'request.json')
        require(observation['id'] == case_id, 'changed case')
        require(observation['state'] in {'complete', 'unavailable', 'timeout', 'invalid_proposal'}, 'unknown state')
        require(request['index_revision'] == manifest['index_revision'], 'changed request index')
        answer = acquisition / case_id / 'answer.txt'
        require(request['answer_sha256'] == digest(answer), 'changed Oracle answer')
        proposals = last_object(answer.read_text()).get('knowledge_queries')
        require(request['proposal'] == proposals, 'changed proposal')
        expected = None
        try:
            require(isinstance(proposals, list) and len(proposals) == 1, 'one proposal per claim')
            expected = query_text(proposals[0])
            if ranker:
                expected = '; '.join(proposals[0]['search_terms'])
        except (ValueError, TypeError, AttributeError):
            require(observation['state'] == 'invalid_proposal', 'invalid proposal was executed')
        require(request['query'] == expected, 'query differs from the recorded mathematical extraction')
        if expected is not None:
            require(digest(folder / 'request.json') == observation['request_sha256'], 'changed query request')
            require(digest(folder / 'response.json') == observation['response_sha256'], 'changed response')
        if observation['state'] == 'complete':
            require(expected is not None and observation.get('exit_code') == 0, 'unsuccessful query marked complete')
            response = read(folder / 'response.json')
            validate_response(response, manifest['snapshot'], manifest['index_revision'])
            require(response['result'].get('ranking_protocol') == ranker, 'response from wrong ranker')
            responses[case_id] = response
        observations.append({'id': case_id, 'state': observation['state'], 'latency_ms': observation.get('latency_ms')})
    result = assess(cases, read(root / 'sources.json'), observations, responses)
    result.update(freeze_sha256=freeze, manifest_sha256=digest(run / 'manifest.json'), ranking_protocol=ranker)
    return result


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--corpus', type=Path, default=Path(__file__).parent / 'coverage_v1')
    for name in ['acquisition', 'initial', 'run']:
        parser.add_argument('--' + name, type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(score_run(args.corpus, args.acquisition, args.initial, args.run), indent=2))
