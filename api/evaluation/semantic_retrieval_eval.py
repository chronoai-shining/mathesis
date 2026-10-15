#!/usr/bin/env python3
"""Execute completed Oracle proposals once on the same admitted source index.

No model call occurs here. Unfinished acquisitions and existing query intents
are left intact; no successful production receipt is invented.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import time

from coverage_eval import digest, read, require
from normalize_oracle import last_object
from normalize_source_oracle import normalize_sources
from retrieval_eval import validate_response, write_new


def query_text(proposal):
    require(proposal.get('claim') == 'C1', 'wrong confirmed claim')
    for field, minimum, maximum in [('definitions', 0, 30), ('hypotheses', 0, 30), ('search_terms', 1, 20)]:
        value = proposal.get(field)
        require(isinstance(value, list) and minimum <= len(value) <= maximum, 'invalid ' + field)
        require(all(isinstance(s, str) and s.strip() and len(s.encode()) <= 2000 for s in value), 'invalid ' + field + ' entry')
    for field, maximum in [('quantifiers', 6000), ('conclusion', 6000), ('reason', 2000)]:
        value = proposal.get(field)
        require(isinstance(value, str) and value.strip() and len(value.encode()) <= maximum, 'invalid ' + field)
    query = ('Definitions: ' + '; '.join(proposal['definitions'])
        + '\nHypotheses: ' + '; '.join(proposal['hypotheses'])
        + '\nQuantifiers: ' + proposal['quantifiers']
        + '\nConclusion: ' + proposal['conclusion']
        + '\nRelated mathematical terms: ' + '; '.join(proposal['search_terms']))
    require(len(query.encode()) <= 30000, 'query exceeds worker byte budget')
    return query


def acquire(args):
    data, freeze, oracle_run = normalize_sources(args.acquisition, args.retrieval, args.corpus)
    args.output.mkdir(parents=True, exist_ok=True)
    source_manifest = read(args.retrieval / 'manifest.json')
    manifest = {'schema': 'mathesis.semantic-retrieval-run.v1', 'freeze_sha256': freeze,
        'index_revision': source_manifest['index_revision'], 'snapshot': source_manifest['snapshot'],
        'binary_sha256': digest(args.binary), 'initial_retrieval_manifest_sha256': digest(args.retrieval / 'manifest.json'),
        'acquisition_metadata_sha256': digest(args.acquisition / 'metadata.json'),
        'scope': 'Actual lexical queries from untrusted Oracle mathematical reformulations; no CMA source audit or review receipt'}
    if args.ranking_protocol:
        manifest['ranking_protocol'] = args.ranking_protocol
        manifest['scope'] = 'Actual standard-term queries with pinned identifier/path ranking; complete original proposal retained; no CMA audit or review receipt'
    if args.revision:
        manifest['implementation_revision'] = args.revision
    if (args.output / 'manifest.json').exists():
        require(read(args.output / 'manifest.json') == manifest, 'semantic acquisition inputs changed')
    else:
        write_new(args.output / 'manifest.json', manifest)
    environment = {k: v for k, v in os.environ.items() if k in {'PATH', 'HOME', 'TMPDIR', 'SSL_CERT_FILE'}}
    environment.update(WISHPOOL_STORAGE='mongo', WISHPOOL_AUTH_MODE='dev', WISHPOOL_BIND='127.0.0.1:0',
        WISHPOOL_PUBLIC_URL='http://127.0.0.1:8080', WISHPOOL_MONGODB_URI=args.mongo_uri, WISHPOOL_MONGODB_DATABASE=args.database)
    for prediction in oracle_run['predictions']:
        case_id = prediction['id']
        folder = args.output / case_id
        if folder.exists():
            continue  # Original intent is authoritative even if a prior process was interrupted.
        folder.mkdir()
        report = last_object((args.acquisition / case_id / 'answer.txt').read_text())
        proposals = report.get('knowledge_queries')
        query = None
        reason = None
        try:
            require(isinstance(proposals, list) and len(proposals) == 1, 'exactly one proposal per confirmed claim required')
            query = query_text(proposals[0])
            if args.ranking_protocol:
                query = '; '.join(proposals[0]['search_terms'])
        except (ValueError, AttributeError) as error:
            reason = str(error)
        request = {'proposal': proposals, 'query': query, 'index_revision': manifest['index_revision'],
            'answer_sha256': digest(args.acquisition / case_id / 'answer.txt')}
        write_new(folder / 'request.json', request)
        if query is None:
            write_new(folder / 'observation.json', {'id': case_id, 'state': 'invalid_proposal', 'reason': reason})
            continue
        started = time.monotonic_ns()
        observation = {'id': case_id, 'state': 'unavailable', 'request_sha256': digest(folder / 'request.json')}
        with (folder / 'response.json').open('xb') as out, (folder / 'stderr.txt').open('xb') as err:
            try:
                command = [str(args.binary.resolve()), '--knowledge-query', manifest['index_revision'], query]
                if args.ranking_protocol:
                    command.append(args.ranking_protocol)
                completed = subprocess.run(command,
                    env=environment, stdout=out, stderr=err, timeout=180, check=False)
                observation['exit_code'] = completed.returncode
            except subprocess.TimeoutExpired:
                observation['state'] = 'timeout'
        observation['latency_ms'] = (time.monotonic_ns() - started) // 1_000_000
        observation['response_sha256'] = digest(folder / 'response.json')
        if observation.get('exit_code') == 0:
            response = read(folder / 'response.json')
            validate_response(response, manifest['snapshot'], manifest['index_revision'])
            require(response['result'].get('ranking_protocol') == args.ranking_protocol, 'query used another ranker')
            observation['state'] = 'complete'
        write_new(folder / 'observation.json', observation)
        print(case_id, observation['state'], observation['latency_ms'], flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--corpus', type=Path, default=Path(__file__).parent / 'coverage_v1')
    for arg in ['acquisition','retrieval','output','binary']:
        parser.add_argument('--' + arg, type=Path, required=True)
    parser.add_argument('--mongo-uri', required=True)
    parser.add_argument('--database', required=True)
    parser.add_argument('--ranking-protocol', choices=['mathesis.source-ranker.identifier-paths.v1'])
    parser.add_argument('--revision')
    acquire(parser.parse_args())
