import { render, screen } from '@testing-library/react';
import { expect, it } from 'vitest';
import type { ComputationReceipt } from '../api/types';
import { queuedRound } from '../test/referee';
import { ComputationEvidence } from './ComputationEvidence';

const receipt: ComputationReceipt = {
  protocol: 'mathesis.rational-projection-amgm.v1',
  round: 1,
  version: 2,
  claims_revision: 3,
  source_sha256: 'source',
  attachment: {
    path: 'projection.json',
    kind: 'computation',
    description: 'Projection counterexample',
    claims: ['C1'],
    sha256: 'input-hash',
    bytes: 123,
  },
  state: 'counterexample',
  dimension: 3,
  numerator: '-8',
  denominator: '243',
  explanation:
    'Negative for this matrix expression. A separate source audit must establish correspondence to the manuscript.',
  checked_at: '2026-10-16T00:00:00Z',
};

it('shows the exact calculation and source limits without assigning manuscript correctness', () => {
  const { rerender } = render(
    <ComputationEvidence
      round={{ ...queuedRound, computation_contract: 1, computation_receipts: [receipt] }}
    />,
  );
  expect(screen.getByText('Exact quadratic form: -8/243')).toBeInTheDocument();
  expect(screen.getByText(/Counterexample to the checked matrix inequality/)).toBeInTheDocument();
  expect(screen.getByText('Declared claim references: C1')).toBeInTheDocument();
  expect(screen.getByText(/source audit must establish whether/)).toBeInTheDocument();
  rerender(
    <ComputationEvidence
      round={{
        ...queuedRound,
        computation_receipts: [
          { ...receipt, state: 'arithmetic_limit', numerator: null, denominator: null },
        ],
      }}
    />,
  );
  expect(screen.getByText(/Exact arithmetic limit reached/)).toBeInTheDocument();
  expect(screen.queryByText(/Exact quadratic form/)).toBeNull();
});

it('keeps old rounds without actual replay receipts empty', () => {
  const { container } = render(<ComputationEvidence round={queuedRound} />);
  expect(container).toBeEmptyDOMElement();
});
