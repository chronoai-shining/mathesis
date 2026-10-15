#!/usr/bin/env python3
"""Compare source rankers on frozen development inputs and saved Oracle queries.

Labels are read only by scoring, never used to construct queries. This is an
offline diagnostic, not a model call, production receipt or entailment judgment.
"""
import argparse
import datetime
import gzip
import hashlib
import json
from pathlib import Path
import subprocess

from coverage_eval import corpus, digest, read, require
from normalize_oracle import last_object
from retrieval_eval import assess, validate_response, write_new
from semantic_retrieval_eval import query_text
from source_referee_eval import parse_stream

ALGORITHMS = ['legacy', 'identifier_components', 'identifier_fields']


def prepare(cases, acquisition):
    queries, inputs, missing = [], {}, []
    for case in cases:
        require(case['split'] == 'development', 'ranking accepts development inputs only')
        case_id = case['id']
        queries.append({'id': case_id + ':raw', 'query': 'Question\n' + case['input']['statement']})
        answer = acquisition / case_id / 'answer.txt'
        receipt_file = answer.parent / 'receipt.json'
        if not answer.exists() or not receipt_file.exists():
            missing.append(case_id)
            continue
        receipt = read(receipt_file)
        require(receipt['case_id'] == case_id, 'changed case identity')
        require(digest(answer.parent / 'request.json') == receipt['request_sha256'], 'changed Oracle request')
        require(digest(answer.parent / 'response.sse') == receipt['response_sha256'], 'changed Oracle stream')
        if receipt['exit_code'] != 0 or not receipt['stream_done'] or receipt['errors']:
            missing.append(case_id)
            continue
        streamed, _, errors, done = parse_stream((answer.parent / 'response.sse').read_text())
        require(done and not errors and streamed == answer.read_text(), 'answer differs from completed stream')
        try:
            proposals = last_object(answer.read_text()).get('knowledge_queries')
            require(isinstance(proposals, list) and len(proposals) == 1, 'one proposal required')
            full = query_text(proposals[0])
        except (ValueError, TypeError, AttributeError):
            missing.append(case_id)
            continue
        inputs[case_id] = {'answer_sha256': digest(answer), 'proposal': proposals[0]}
        queries.extend([
            {'id': case_id + ':semantic', 'query': full},
            {'id': case_id + ':terms', 'query': '; '.join(proposals[0]['search_terms'])},
        ])
    return queries, inputs, missing


def score_results(cases, sources, queries, result, snapshot, index):
    require(result.get('diagnostic_only') is True, 'not diagnostic output')
    rows = result['results']
    expected = {q['id']: q['query'] for q in queries}
    require(len(expected) == len(queries), 'duplicate query id')
    require(len(rows) == len(expected) and {r['id'] for r in rows} == set(expected),
            'missing, duplicate or extra ranking result')
    for row in rows:
        require(row['query'] == expected[row['id']], 'query bytes changed')
        validate_response({'diagnostic_only': True, 'result': row['result']}, snapshot, index)
    scores = {}
    for kind in ['raw', 'semantic', 'terms']:
        selected = {r['id'].removesuffix(':' + kind): r for r in rows if r['id'].endswith(':' + kind)}
        observations = [{'id': c['id'], 'state': 'complete' if c['id'] in selected else 'unavailable',
                         'latency_ms': selected[c['id']]['query_ms'] if c['id'] in selected else None}
                        for c in cases]
        responses = {key: {'result': row['result']} for key, row in selected.items()}
        scores[kind] = assess(cases, sources, observations, responses)
    return scores


def acquire(args):
    data, freeze = corpus(args.corpus)
    cases = [c for c in data['cases'] if c['split'] == 'development']
    queries, inputs, missing = prepare(cases, args.acquisition)
    require(args.allow_partial or not missing, 'wait for all usable Oracle proposals, or explicitly allow partial')
    args.output.mkdir(parents=True, exist_ok=False)
    write_new(args.output / 'queries.json', queries)
    manifest = {'schema': 'mathesis.ranking-run.v1', 'split': 'development',
                'started_at_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
                'freeze_sha256': freeze, 'implementation_revision': args.revision,
                'binary_sha256': digest(args.binary), 'artifact_sha256': digest(args.index_file),
                'index_revision': args.index_revision, 'snapshot': data['source_snapshot']['digest'],
                'queries_sha256': digest(args.output / 'queries.json'),
                'oracle_inputs': inputs, 'missing_proposals': missing,
                'limits': ['Development only; provisional labels, no human adjudication.',
                           'Exact predeclared source-set recall, not usefulness of other results.',
                           'No source audit, model-quality comparison or production receipt.',
                           'Local index load and warm query times exclude Mongo and network.']}
    write_new(args.output / 'manifest.json', manifest)
    sources = read(args.corpus / 'sources.json')
    for algorithm in ALGORITHMS:
        path = args.output / (algorithm + '.json')
        with (args.output / 'queries.json').open('rb') as stdin, path.open('xb') as stdout:
            subprocess.run([str(args.binary.resolve()), str(args.index_file.resolve()), algorithm],
                           stdin=stdin, stdout=stdout, check=True, timeout=600)
        result = read(path)
        require(result['algorithm'] == algorithm, 'wrong ranker')
        scores = score_results(cases, sources, queries, result, manifest['snapshot'], args.index_revision)
        scores['load_ms'] = result['load_ms']
        scores['response_sha256'] = digest(path)
        # Retain exact output bytes, compressed only to avoid repeating public source text.
        with (args.output / (algorithm + '.json.gz')).open('xb') as out:
            out.write(gzip.compress(path.read_bytes(), mtime=0))
        write_new(args.output / (algorithm + '-score.json'), scores)
        print(algorithm, {kind: scores[kind]['required_declarations_at_6']
                          for kind in ['raw', 'semantic', 'terms']}, flush=True)


def verify(args):
    data, freeze = corpus(args.corpus)
    manifest = read(args.run / 'manifest.json')
    require(manifest['split'] == 'development' and manifest['freeze_sha256'] == freeze, 'changed corpus')
    require(manifest['snapshot'] == data['source_snapshot']['digest'], 'changed snapshot')
    require(digest(args.run / 'queries.json') == manifest['queries_sha256'], 'changed query batch')
    cases = [c for c in data['cases'] if c['split'] == 'development']
    queries = read(args.run / 'queries.json')
    require(set(manifest['oracle_inputs']) <= {c['id'] for c in cases}, 'proposal outside development split')
    require(set(manifest['missing_proposals']) == {c['id'] for c in cases} - set(manifest['oracle_inputs']), 'wrong missing-proposal inventory')
    reconstructed = []
    for case in cases:
        case_id = case['id']
        reconstructed.append({'id': case_id + ':raw', 'query': 'Question\n' + case['input']['statement']})
        if case_id in manifest['oracle_inputs']:
            proposal = manifest['oracle_inputs'][case_id]['proposal']
            reconstructed.extend([
                {'id': case_id + ':semantic', 'query': query_text(proposal)},
                {'id': case_id + ':terms', 'query': '; '.join(proposal['search_terms'])},
            ])
    require(queries == reconstructed, 'queries differ from frozen inputs and saved proposals')
    for algorithm in ALGORITHMS:
        saved = read(args.run / (algorithm + '-score.json'))
        raw = gzip.decompress((args.run / (algorithm + '.json.gz')).read_bytes())
        require(hashlib.sha256(raw).hexdigest() == saved['response_sha256'], 'changed exact output')
        result = json.loads(raw)
        require(result['algorithm'] == algorithm, 'wrong ranker')
        scores = score_results(cases, read(args.corpus / 'sources.json'), queries, result,
                               manifest['snapshot'], manifest['index_revision'])
        require(all(saved[k] == v for k, v in scores.items()), 'saved score does not reproduce')
    print('verified exact compressed output and development scores')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--corpus', type=Path, default=Path(__file__).parent / 'coverage_v1')
    commands = parser.add_subparsers(dest='command', required=True)
    acquire_parser = commands.add_parser('acquire')
    for name in ['acquisition', 'binary', 'index-file', 'output']:
        acquire_parser.add_argument('--' + name, type=Path, required=True)
    acquire_parser.add_argument('--index-revision', required=True)
    acquire_parser.add_argument('--revision', required=True)
    acquire_parser.add_argument('--allow-partial', action='store_true')
    verify_parser = commands.add_parser('verify')
    verify_parser.add_argument('run', type=Path)
    arguments = parser.parse_args()
    if arguments.command == 'acquire':
        acquire(arguments)
    else:
        verify(arguments)
