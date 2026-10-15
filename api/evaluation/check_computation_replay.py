#!/usr/bin/env python3
"""Cross-check Rust matrix replay with independent exact rank-one contractions.

No model, upstream execution, Lean or mathematical coverage claim is involved.
"""
import argparse
from fractions import Fraction
import itertools
import json
import random
import subprocess


def expected(vectors, witness):
    dot = lambda a, b: sum(x*y for x, y in zip(a, b))
    norms = [dot(v, v) for v in vectors]
    def contraction(a, b, c):
        return Fraction(dot(witness, vectors[a]) * dot(vectors[a], vectors[b])
                        * dot(vectors[b], vectors[c]) * dot(vectors[c], witness),
                        norms[a] * norms[b] * norms[c])
    return (sum(contraction(*indices) for indices in itertools.product(range(3), repeat=3)) / 27
            - sum(contraction(*indices) for indices in itertools.permutations(range(3))) / 6)


def check(program):
    rng = random.Random(20261016)
    cases = [([[1,-1,-1],[-1,1,-1],[-1,-1,1]], [1,1,1])]
    for dimension in range(1, 7):
        for _ in range(10):
            vectors = []
            while len(vectors) < 3:
                vector = [rng.randint(-5, 5) for _ in range(dimension)]
                if any(vector): vectors.append(vector)
            cases.append((vectors, [rng.randint(-5, 5) for _ in range(dimension)]))
    for vectors, witness in cases:
        request = {'protocol': 'mathesis.rational-projection-amgm.v1',
                   'vectors': vectors, 'witness': witness}
        response = json.loads(subprocess.check_output([program], input=json.dumps(request).encode()))
        value = expected(vectors, witness)
        assert response['numerator'] == str(value.numerator), (request, response, value)
        assert response['denominator'] == str(value.denominator), (request, response, value)
        assert response['state'] == ('counterexample' if value < 0 else 'not_counterexample')
    print(f'{len(cases)} exact cross-checks passed; independent rank-one contraction formula')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('checker')
    check(parser.parse_args().checker)
