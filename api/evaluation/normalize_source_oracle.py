#!/usr/bin/env python3
"""Normalize source-informed Oracle readings; never grant independent audit status."""
import argparse
import json
from pathlib import Path

from coverage_eval import corpus, digest, read, require, score
from normalize_oracle import normalize
from retrieval_eval import score_run, write_new


def normalize_sources(acquisition, retrieval, root):
    data, freeze = corpus(root)
    score_run(root, retrieval)  # Recheck exact inputs, responses and all identities.
    metadata = read(acquisition / 'metadata.json')
    require(metadata['retrieval_manifest_sha256'] == digest(retrieval / 'manifest.json'),
            'source run manifest changed')
    run = normalize(acquisition, data, freeze)
    manifest = read(retrieval / 'manifest.json')
    sources = read(root / 'sources.json')
    families = {source['repo_path']: family for family, source in sources.items()}
    for prediction in run['predictions']:
        folder = retrieval / prediction['id']
        observation = read(folder / 'observation.json')
        require(observation['state'] == 'complete', 'no source query completed')
        prompt = read(acquisition / prediction['id'] / 'request.json')['messages'][0]['content']
        require('evaluation:' + observation['response_sha256'] in prompt,
                'actual query response was not included in this prompt')
        hits = read(folder / 'response.json')['result']['hits'][:6]
        prediction['query_status'] = 'complete'
        prediction['returned_sources'] = [
            {'family': families.get(hit['module_path'], hit['module_path']),
             'statement_id': declaration['identity']['statement_id'],
             'declaration_name_key': declaration['identity']['name_key']}
            for hit in hits for declaration in hit['declarations']]
        prediction['retrieval_latency_ms'] = observation['latency_ms']
    run['method'] = 'contract 3 statement-only Oracle with recorded source queries and knowledge contract 2 reformulation proposals; no CMA audit'
    run['snapshot'] = manifest['snapshot']
    run['index_revision'] = manifest['index_revision']
    run['retrieval_manifest_sha256'] = metadata['retrieval_manifest_sha256']
    run['budget']['initial_candidates_per_case'] = 6
    run['limits'] = ['Diagnostic source results are not admitted production review receipts.',
                     'Reformulation proposals are not counted as executed queries.',
                     'No independent CMA source audit or publication decision.',
                     'Label evidence remains provisional; held-out cases were not queried.']
    return data, freeze, run


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--corpus', type=Path, default=Path(__file__).parent / 'coverage_v1')
    parser.add_argument('--acquisition', type=Path, required=True)
    parser.add_argument('--retrieval', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    data, freeze, run = normalize_sources(args.acquisition, args.retrieval, args.corpus)
    write_new(args.output, run)
    measured = score(data, freeze, run, 'development')
    measured['run_sha256'] = digest(args.output)
    print(json.dumps(measured, indent=2))


if __name__ == '__main__':
    main()
