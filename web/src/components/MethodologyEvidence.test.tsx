import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import type { MethodologyReading } from '../api/types';
import { MethodologyEvidence } from './MethodologyEvidence';

const reading: MethodologyReading = {
  baseline: [{ statement: 'The compactness hypotheses', location: 'Section 1', source: null }],
  searched: ['Compared the explicitly stated prior estimates.'],
  literature: 'supported',
  binding_analysis: 'Instantiation does not improve the estimate.',
  delta: 'A uniform bound for the family.',
  escape: 'content',
  witnesses: [
    {
      kind: 'conclusion',
      statement: 'The uniform bound holds.',
      location: 'Theorem 1',
      non_binding_step: 'Construct a finite subcover.',
      live_use: 'The final estimate consumes the cover.',
    },
  ],
  utility: {
    kind: 'general_theorem',
    status: 'supported',
    rationale: 'Answers the uniform-bound question.',
    problem: '',
    progress: '',
    prerequisites: '',
    discharge: '',
  },
  settlement: null,
  information_gain: null,
  limits: ['The literature comparison is bounded by the reported search.'],
};

describe('methodology evidence', () => {
  it('shows the live conclusion, baseline, value and search limits without a fabricated score', () => {
    render(<MethodologyEvidence reading={reading} />);
    expect(screen.getByText(/Conclusion witness/)).toHaveTextContent('Theorem 1');
    expect(screen.getByText('The final estimate consumes the cover.')).toBeInTheDocument();
    expect(screen.getByText('Answers the uniform-bound question.')).toBeInTheDocument();
    expect(screen.getByText(reading.limits[0]!)).toBeInTheDocument();
    expect(screen.queryByText(/\d+%/)).not.toBeInTheDocument();
    expect(screen.queryByText(/Separate information-gain analysis/)).not.toBeInTheDocument();
  });

  it('presents insufficient evidence as undetermined, independently of mathematical value', () => {
    render(
      <MethodologyEvidence
        reading={{
          ...reading,
          escape: 'undetermined',
          literature: 'insufficient',
          witnesses: [],
          delta: '',
        }}
      />,
    );
    expect(screen.getByText(/New content and mathematical value/)).toHaveTextContent(
      'Undetermined',
    );
    expect(screen.getByText('Literature: Insufficient evidence')).toBeInTheDocument();
    expect(screen.queryByText(/Bind-only/)).not.toBeInTheDocument();
    expect(screen.getByText('Answers the uniform-bound question.')).toBeInTheDocument();
  });
});
