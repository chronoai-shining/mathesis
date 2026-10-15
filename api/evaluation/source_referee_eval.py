#!/usr/bin/env python3
"""Acquire source-informed Oracle development readings, retaining each stream.

No CMA audit or publication is performed. Run only after a checked retrieval
run is available. One streaming call per case; uncertain calls are never retried.
"""
import argparse
import concurrent.futures
import datetime
import json
from pathlib import Path
import subprocess
import time

from coverage_eval import corpus, digest, read, require
from retrieval_eval import score_run, write_new


def parse_stream(text):
    answer, oracle, errors, done = '', {}, [], False
    for line in text.splitlines():
        if not line.startswith('data: '):
            continue
        if line == 'data: [DONE]':
            done = True
            continue
        try:
            chunk = json.loads(line[6:])
        except ValueError:
            errors.append('invalid SSE JSON')
            continue
        if 'error' in chunk:
            errors.append(chunk['error'])
        answer += ''.join((choice.get('delta') or {}).get('content') or ''
                          for choice in chunk.get('choices', []))
        oracle = chunk.get('oracle') or oracle
    return answer, oracle, errors, done


def acquire(args):
    data, freeze = corpus(args.corpus)
    measured = score_run(args.corpus, args.retrieval)
    require(measured['completed']['numerator'] == measured['completed']['denominator'],
            'source-informed run needs complete actual retrieval inputs')
    args.output.mkdir(parents=True, exist_ok=False)
    manifest = read(args.retrieval / 'manifest.json')
    metadata = {'freeze_sha256': freeze, 'implementation_revision': args.revision,
        'scope': 'contract 3 statement-only Oracle with actual source evidence and knowledge contract 2 proposals; no CMA audit or publication',
        'retrieval_manifest_sha256': digest(args.retrieval / 'manifest.json'),
        'retrieval_index_revision': manifest['index_revision'],
        'started_at_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
        'prompt_exporter_sha256': digest(args.exporter), 'parallel_requests': 2,
        'cost_usd': None, 'diagnostic_only': True}
    write_new(args.output / 'metadata.json', metadata)
    requests = []
    # Prepare every exact input before any model request. Labels never enter it.
    for case in data['cases']:
        if case['split'] != 'development':
            continue
        observation = read(args.retrieval / case['id'] / 'observation.json')
        result = read(args.retrieval / case['id'] / 'response.json')['result']
        result['hits'] = result['hits'][:6]
        # Match the worker's additional per-hit prompt prefix bound.
        per_hit = (800_000 - 1024) // max(1, len(result['hits'])) // 2
        omitted = False
        for hit in result['hits']:
            source = hit['source_excerpt'].encode()
            excerpt = source[:per_hit].decode('utf-8', errors='ignore')
            if excerpt != hit['source_excerpt']:
                hit['excerpt_truncated'] = True
                omitted = True
            hit['source_excerpt'] = excerpt
        context = {'protocol': 2, 'diagnostic_only': True,
            'cma_followup_used': False, 'missing_semantic_claims': ['C1'],
            'receipts': [{
                'receipt_id': 'evaluation:' + observation['response_sha256'], 'claim': 'C1',
                'snapshot': result['snapshot'], 'index_revision': result['index_revision'],
                'state': 'complete', 'response_identity': {'sha256': observation['response_sha256']},
                'scope': ['diagnostic full-source term search; top six candidates from the saved top-ten response'],
                'limits': result['limits'], 'prompt_evidence_limited': omitted, 'result': result}],
            'limits': ['Actual diagnostic queries on the frozen publication; these ids are not production review receipts.',
                'Candidate retrieval does not establish entailment or novelty. Read every necessary hypothesis.',
                'The exact saved response was reduced to its first six candidates; external literature remains required.']}
        folder = args.output / case['id']
        folder.mkdir()
        request = subprocess.check_output([str(args.exporter.resolve())], input=json.dumps({
            'id': case['id'], 'statement': case['input']['statement'], 'knowledge': context}).encode())
        with (folder / 'request.json').open('xb') as target:
            target.write(request)
        requests.append((case['id'], folder))

    def run(item):
        case_id, folder = item
        request, output = folder / 'request.json', folder / 'response.sse'
        started = time.monotonic_ns()
        print('START', case_id, flush=True)
        with output.open('xb') as out, (folder / 'stderr.log').open('xb') as err:
            result = subprocess.run(['nyxid', 'proxy', 'request', 'oracle',
                'api/v1/oracle/openai/v1/chat/completions', '--method', 'POST',
                '--data', '@' + str(request.resolve())], stdout=out, stderr=err, check=False)
        answer, oracle, errors, done = parse_stream(output.read_text())
        with (folder / 'answer.txt').open('x') as target:
            target.write(answer)
        write_new(folder / 'receipt.json', {'case_id': case_id, 'exit_code': result.returncode,
            'stream_done': done, 'errors': errors, 'oracle': oracle,
            'elapsed_ms': (time.monotonic_ns() - started) // 1_000_000,
            'request_sha256': digest(request), 'response_sha256': digest(output)})
        print('FINISH', case_id, result.returncode, done, oracle.get('task_id'), flush=True)

    with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
        list(pool.map(run, requests))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--corpus', type=Path, default=Path(__file__).parent / 'coverage_v1')
    parser.add_argument('--retrieval', type=Path, required=True)
    parser.add_argument('--exporter', type=Path, required=True)
    parser.add_argument('--revision', required=True)
    parser.add_argument('--output', type=Path, required=True)
    acquire(parser.parse_args())


if __name__ == '__main__':
    main()
