#!/usr/bin/env python3
"""Verify the retained single-case computation diagnostic without provider I/O."""
import argparse
import json
from pathlib import Path
import subprocess

from check_computation_replay import expected
from coverage_eval import corpus, digest, read, require
from normalize_oracle import last_object
from source_referee_eval import parse_stream


def verify(folder, checker=None):
    root = Path(__file__).parent
    data, freeze = corpus(root / 'coverage_v1')
    metadata = read(folder / 'metadata.json')
    require(metadata['freeze_sha256'] == freeze, 'corpus freeze mismatch')
    require(metadata['case_id'] == 'projection_amgm-original', 'wrong diagnostic case')
    case = next(c for c in data['cases'] if c['id'] == metadata['case_id'])
    require(case['split'] == metadata['split'] == 'development', 'not a development case')
    baseline = root / 'runs/source-oracle-development-20261016' / case['id']
    require(digest(baseline / 'request.json') == metadata['baseline_request_sha256'],
            'baseline request changed')
    require(digest(baseline / 'receipt.json') == metadata['baseline_receipt_sha256'],
            'baseline receipt changed')
    for name, field in [('request.json', 'request_sha256'),
                        ('projection.json', 'attachment_sha256'),
                        ('replay.json', 'replay_sha256')]:
        require(digest(folder / name) == metadata[field], f'{name} changed')
    attachment, replay = read(folder / 'projection.json'), read(folder / 'replay.json')
    value = expected(attachment['vectors'], attachment['witness'])
    require((replay['numerator'], replay['denominator']) ==
            (str(value.numerator), str(value.denominator)), 'independent arithmetic mismatch')
    require(value < 0 and replay['state'] == 'counterexample', 'no negative witness')
    if checker:
        actual = json.loads(subprocess.check_output(
            [str(checker.resolve())], input=(folder / 'projection.json').read_bytes()))
        require(actual == replay, 'checker output changed')
    request = read(folder / 'request.json')
    original = read(baseline / 'request.json')
    text = request['messages'][0]['content']
    start = text.index('\nDiagnostic declared computation attachment')
    end = text.index('\nSystem-executed knowledge queries', start)
    require(text[:start] + text[end:] == original['messages'][0]['content'],
            'baseline prompt changed outside the finite-evidence insertion')
    context = read(folder / 'context.json')
    require(json.dumps(context, ensure_ascii=False, separators=(',', ':')) in text[start:end],
            'saved context missing from request')
    require(context['receipts'][0]['result'] == replay, 'context replay mismatch')
    require(context['receipts'][0]['attachment_sha256'] == metadata['attachment_sha256'],
            'context attachment mismatch')
    request['messages'][0]['content'] = original['messages'][0]['content']
    require(request == original, 'request envelope changed')
    readings = []
    for directory in [baseline, folder]:
        receipt = read(directory / 'receipt.json')
        require(receipt['request_sha256'] == digest(directory / 'request.json'), 'request hash')
        require(receipt['response_sha256'] == digest(directory / 'response.sse'), 'stream hash')
        answer, oracle, errors, done = parse_stream((directory / 'response.sse').read_text())
        require(receipt['exit_code'] == 0 and done and not errors, 'incomplete transport')
        require(receipt['stream_done'] == done and receipt['errors'] == errors and
                receipt['oracle'] == oracle, 'receipt differs from stream')
        require(answer == (directory / 'answer.txt').read_text(), 'answer differs from stream')
        report = last_object(answer)
        require(report is not None and len(report.get('claims', [])) == 1, 'unusable report')
        require(report['claims'][0].get('claim') == 'C1', 'claim mismatch')
        reading = report['claims'][0].get('conjecture', {})
        readings.append({'status': reading.get('status'), 'escape': reading.get('escape'),
                         'observed_model': oracle.get('observed_model_switcher'),
                         'observed_effort': oracle.get('observed_model_effort'),
                         'elapsed_ms': receipt['elapsed_ms']})
    return {'case_id': case['id'], 'exact_value': str(value),
            'before': readings[0], 'with_replay': readings[1],
            'qualification': 'One selected development Oracle reading; no independent CMA audit'}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--run', type=Path, default=Path(__file__).parent /
                        'runs/computation-oracle-development-20261016')
    parser.add_argument('--checker', type=Path)
    args = parser.parse_args()
    print(json.dumps(verify(args.run, args.checker), indent=2))
