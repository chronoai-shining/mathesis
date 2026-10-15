import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import type { ClaimCoverage, KnowledgeQueryReceipt } from '../api/types';
import { queuedRound } from '../test/referee';
import { CoverageEvidence, KnowledgeQueries } from './KnowledgeEvidence';

const receipt: KnowledgeQueryReceipt = {
  id: 'query:1',
  protocol: 'mathesis.artifact-query.v1',
  subject: {
    submission: 'private-paper',
    version: 1,
    claims_revision: 2,
    round: 1,
    claim: 'C1',
    source_digest: 'source',
    claim_digest: 'claim',
  },
  snapshot: 'snapshot',
  index_revision: 'index',
  request_digest: 'request',
  query: 'finite fibers',
  scopes: ['bounded source search'],
  state: 'timeout',
  returned: [],
  limits: ['Deadline exceeded.'],
  retryable: true,
  started_at: '2026-10-16T00:00:00Z',
  completed_at: '2026-10-16T00:02:00Z',
};

describe('private knowledge evidence', () => {
  it('distinguishes timeout from a completed search with no matches', () => {
    const { rerender } = render(
      <KnowledgeQueries
        submissionId="private-paper"
        round={{
          ...queuedRound,
          knowledge_contract: 1,
          knowledge_events: [{ event: 'query_completed', receipt }],
        }}
      />,
    );
    expect(screen.getByText(/knowledge evidence is incomplete/)).toBeInTheDocument();
    expect(screen.getByText(/C1 · Search timed out/)).toBeInTheDocument();
    expect(screen.getByText(/new review on this manuscript version/)).toBeInTheDocument();
    expect(screen.queryByText(/No candidate was returned/)).toBeNull();
    rerender(
      <KnowledgeQueries
        submissionId="private-paper"
        round={{
          ...queuedRound,
          knowledge_contract: 1,
          knowledge_events: [
            {
              event: 'query_completed',
              receipt: {
                ...receipt,
                state: 'complete',
                retryable: false,
                response: { id: 'blob', sha256: 'hash', bytes: 2 },
              },
            },
          ],
        }}
      />,
    );
    expect(screen.getByText(/No candidate was returned/)).toBeInTheDocument();
    expect(screen.getByText(/does not establish novelty/)).toBeInTheDocument();
    expect(
      screen.getByRole('link', { name: 'Download the saved source evidence' }),
    ).toHaveAttribute('href', '/api/v1/submissions/private-paper/referee/knowledge/query%3A1');
  });
  it('shows unresolved hypotheses without upgrading the candidate to certified coverage', () => {
    const coverage: ClaimCoverage = {
      subject: receipt.subject,
      receipt_ids: [receipt.id],
      relation: 'candidate',
      qualification: { level: 'candidate' },
      summary: 'A possible specialization.',
      limits: ['Positivity is missing.'],
      binding: {
        premises: [
          {
            snapshot: 'snapshot',
            module_path: 'Bound.lean',
            declaration_name_key: 'bound',
            statement_id: 'statement',
            type_digest: 'type',
          },
        ],
        relation: 'instance',
        parameters: [{ parameter: 'k', expression: 'n' }],
        hypotheses: [{ hypothesis: 'n > 0', argument: '' }],
        argument: 'Apply the bound if positivity is established.',
        unresolved: ['Prove n > 0.'],
        source_lineage: [],
      },
    };
    render(<CoverageEvidence coverage={coverage} />);
    expect(screen.getByText('Unresolved hypothesis')).toBeInTheDocument();
    expect(screen.getByText('Prove n > 0.')).toBeInTheDocument();
    expect(screen.getByText(/Uncertified proposal/)).toBeInTheDocument();
    expect(screen.getByText(/Joint premise set/)).toBeInTheDocument();
  });
  it('does not fabricate retrospective search evidence for legacy rounds', () => {
    const { container } = render(
      <KnowledgeQueries submissionId="private-paper" round={queuedRound} />,
    );
    expect(container).toBeEmptyDOMElement();
  });
});
