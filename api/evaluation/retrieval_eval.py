#!/usr/bin/env python3
"""Measure source candidate discovery on frozen development inputs only.

The binary's diagnostic command loads the admitted index from Mongo/GridFS.
These observations are not worker query receipts or mathematical judgments.
"""
import argparse
import datetime
import json
import os
from pathlib import Path
import subprocess
import time

from coverage_eval import corpus, digest, read, require, ratio


def write_new(path, value):
    with path.open('x') as output:
        json.dump(value, output, ensure_ascii=False, indent=2)
        output.write('\n')


def assess(cases, sources, observations, responses):
    by_id = {o['id']: o for o in observations}
    require(len(by_id) == len(observations), 'duplicate retrieval observation')
    require(set(by_id) == {c['id'] for c in cases}, 'missing or extra observations')
    rows = []
    for case in cases:
        observation = by_id[case['id']]
        response = responses.get(case['id'])
        hits = response['result']['hits'] if response is not None else []
        expected = case['label']['sources']
        # Both declaration coordinates must match; a module hit alone is weaker.
        declaration_ranks = []
        module_ranks = []
        for wanted in expected:
            module = sources[wanted['family']]['repo_path']
            module_ranks.append(next((rank for rank, hit in enumerate(hits, 1)
                                      if hit['module_path'] == module), None))
            declaration_ranks.append(next((rank for rank, hit in enumerate(hits, 1)
                if hit['module_path'] == module and any(
                    d['identity']['statement_id'] == wanted['statement_id']
                    and d['identity']['name_key'] == wanted['declaration_name_key']
                    for d in hit['declarations'])), None))
        rows.append({
            'id': case['id'], 'family': case['family'],
            'query_state': observation['state'], 'latency_ms': observation['latency_ms'],
            'required_source_count': len(expected),
            'required_declaration_ranks': declaration_ranks,
            'required_module_ranks': module_ranks,
            'all_required_declarations_at_6': bool(expected) and all(
                rank is not None and rank <= 6 for rank in declaration_ranks),
            'all_required_declarations_at_10': bool(expected) and all(
                rank is not None and rank <= 10 for rank in declaration_ranks),
            'all_required_modules_at_6': bool(expected) and all(
                rank is not None and rank <= 6 for rank in module_ranks),
        })
    def metric(field, selected):
        eligible = [row for row in selected if row['required_source_count']]
        return ratio(sum(row[field] for row in eligible), len(eligible))
    return {
        'schema': 'mathesis.retrieval-score.v1',
        'scope': 'candidate retrieval only; no coverage or refutation judgment',
        'observations': len(rows),
        'completed': ratio(sum(row['query_state'] == 'complete' for row in rows), len(rows)),
        'required_declarations_at_6': metric('all_required_declarations_at_6', rows),
        'required_declarations_at_10': metric('all_required_declarations_at_10', rows),
        'required_modules_at_6': metric('all_required_modules_at_6', rows),
        'per_family': {family: {
            'required_declarations_at_6': metric('all_required_declarations_at_6',
                [row for row in rows if row['family'] == family])
        } for family in sorted({row['family'] for row in rows})},
        'cases': rows,
    }


def validate_response(response, snapshot, revision):
    require(response.get('diagnostic_only') is True, 'not a diagnostic response')
    result = response['result']
    require(result['snapshot'] == snapshot, 'changed source snapshot')
    require(result['index_revision'] == revision, 'changed index revision')
    for hit in result['hits']:
        for declaration in hit['declarations']:
            require(declaration['identity']['snapshot'] == snapshot, 'mixed declaration snapshot')
            require(declaration['identity']['module_path'] == hit['module_path'], 'mixed module')
            require(declaration['source_sha256'] == hit['source_sha256'], 'mixed source hash')


def acquire(args):
    data, freeze = corpus(args.corpus)
    cases = [case for case in data['cases'] if case['split'] == 'development']
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    binary = args.binary.resolve()
    manifest = {
        'schema': 'mathesis.retrieval-run.v1', 'split': 'development',
        'freeze_sha256': freeze, 'implementation_revision': args.revision,
        'binary_sha256': digest(binary), 'index_revision': args.index,
        'snapshot': data['source_snapshot']['digest'],
        'started_at_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
        'method': 'Question label plus confirmed statement; diagnostic top 10, scored at 6 and 10',
        'limits': ['No provider call or model judgment.',
                   'Diagnostic responses are not admitted review receipts.',
                   'Per-query latency includes a cold Mongo/GridFS load.',
                   'No held-out case queried; no human-adjudicated labels.'],
    }
    write_new(output / 'manifest.json', manifest)
    environment = {k: v for k, v in os.environ.items()
                   if k in {'PATH', 'HOME', 'TMPDIR', 'SSL_CERT_FILE'}}
    environment.update(WISHPOOL_STORAGE='mongo', WISHPOOL_AUTH_MODE='dev',
        WISHPOOL_BIND='127.0.0.1:0', WISHPOOL_PUBLIC_URL='http://127.0.0.1:8080',
        WISHPOOL_MONGODB_URI=args.mongo_uri,
        WISHPOOL_MONGODB_DATABASE=args.database)
    observations = []
    for case in cases:
        folder = output / case['id']
        folder.mkdir()
        query = 'Question\n' + case['input']['statement']
        require(len(query.encode()) <= 30_000, 'query exceeds worker input bound')
        request = {'index_revision': args.index, 'query': query, 'diagnostic_only': True}
        write_new(folder / 'request.json', request)
        started = time.monotonic_ns()
        observation = {'id': case['id'], 'state': 'unavailable', 'latency_ms': None,
                       'request_sha256': digest(folder / 'request.json')}
        with (folder / 'response.json').open('xb') as stdout, (folder / 'stderr.txt').open('xb') as stderr:
            try:
                completed = subprocess.run([str(binary), '--knowledge-query', args.index, query],
                    env=environment, stdout=stdout, stderr=stderr, timeout=180, check=False)
                observation['exit_code'] = completed.returncode
            except subprocess.TimeoutExpired:
                observation['state'] = 'timeout'
        observation['latency_ms'] = (time.monotonic_ns() - started) // 1_000_000
        observation['response_sha256'] = digest(folder / 'response.json')
        if observation.get('exit_code') == 0:
            response = read(folder / 'response.json')
            validate_response(response, manifest['snapshot'], args.index)
            observation['state'] = 'complete'
        write_new(folder / 'observation.json', observation)
        observations.append(observation)
        print(case['id'], observation['state'], observation['latency_ms'], flush=True)
    write_new(output / 'observations.json', observations)
    write_new(output / 'score.json', score_run(args.corpus, output))


def score_run(root, folder):
    data, freeze = corpus(root)
    manifest = read(folder / 'manifest.json')
    require(manifest['freeze_sha256'] == freeze, 'changed corpus')
    require(manifest['split'] == 'development', 'this diagnostic accepts development only')
    observations = read(folder / 'observations.json')
    cases = [case for case in data['cases'] if case['split'] == 'development']
    inputs = {case['id']: 'Question\n' + case['input']['statement'] for case in cases}
    require(manifest['snapshot'] == data['source_snapshot']['digest'], 'wrong source snapshot')
    responses = {}
    for observation in observations:
        require(observation['id'] in inputs, 'observation outside development split')
        require(observation['state'] in {'complete', 'unavailable', 'timeout'}, 'invalid query state')
        location = folder / observation['id']
        require(location.resolve().is_relative_to(folder.resolve()), 'observation path escapes run')
        require(digest(location / 'response.json') == observation['response_sha256'], 'changed response')
        require(digest(location / 'request.json') == observation['request_sha256'], 'changed request')
        request = read(location / 'request.json')
        require(request['query'] == inputs[observation['id']], 'query differs from frozen input')
        require(request['index_revision'] == manifest['index_revision'], 'request changed index')
        if observation['state'] == 'complete':
            response = read(location / 'response.json')
            validate_response(response, manifest['snapshot'], manifest['index_revision'])
            responses[observation['id']] = response
    cases = [case for case in data['cases'] if case['split'] == 'development']
    result = assess(cases, read(Path(root) / 'sources.json'), observations, responses)
    result['freeze_sha256'] = freeze
    result['manifest_sha256'] = digest(folder / 'manifest.json')
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--corpus', type=Path, default=Path(__file__).parent / 'coverage_v1')
    commands = parser.add_subparsers(dest='command', required=True)
    acquisition = commands.add_parser('acquire')
    acquisition.add_argument('--binary', type=Path, required=True)
    acquisition.add_argument('--revision', required=True)
    acquisition.add_argument('--index', required=True)
    acquisition.add_argument('--mongo-uri', required=True)
    acquisition.add_argument('--database', required=True)
    acquisition.add_argument('--output', type=Path, required=True)
    scoring = commands.add_parser('score')
    scoring.add_argument('run', type=Path)
    args = parser.parse_args()
    if args.command == 'acquire':
        acquire(args)
    else:
        print(json.dumps(score_run(args.corpus, args.run), indent=2))


if __name__ == '__main__':
    main()
