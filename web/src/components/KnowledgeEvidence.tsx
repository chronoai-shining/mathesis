import type { ClaimCoverage, KnowledgeQueryReceipt, RefereeRound } from '../api/types';
import { Markdown } from './Markdown';

const states: Record<KnowledgeQueryReceipt['state'], string> = {
  complete: 'Recorded search completed',
  partial: 'Search incomplete',
  timeout: 'Search timed out',
  unavailable: 'Knowledge source unavailable',
  invalid_publication: 'Knowledge source failed validation',
};
const relations: Record<ClaimCoverage['relation'], string> = {
  unknown: 'No established relation in the inspected evidence',
  candidate: 'Candidate relation — further work needed',
  covered: 'Covered by the cited results',
  refuted: 'Refuted by the cited results',
};

export function KnowledgeQueries({
  round,
  submissionId,
}: {
  round: RefereeRound;
  submissionId: string;
}) {
  if (!round.knowledge_contract) return null;
  const events = round.knowledge_events ?? [];
  const receipts = events.flatMap((e) => (e.event === 'query_completed' ? [e.receipt] : []));
  const pending = events.filter(
    (e) => e.event === 'query_admitted' && !receipts.some((r) => r.id === e.intent.id),
  );
  const incomplete =
    receipts.some((r) => r.state !== 'complete') || pending.length > 0 || receipts.length === 0;
  return (
    <section aria-label="Knowledge search evidence" className="report">
      <h3>Knowledge search</h3>
      <p>
        {incomplete
          ? 'Required knowledge evidence is incomplete. This search cannot establish sufficient novelty evidence.'
          : 'The recorded bounded searches completed. Candidate matches still require a mathematical comparison.'}
      </p>
      <p className="hint">
        A search with no matches does not establish novelty. External literature remains part of the
        review.
      </p>
      {receipts.some((r) => r.retryable) ? (
        <p>
          The source can be retried after the recorded limitation is resolved. Staff can start a new
          review on this manuscript version; the earlier evidence is retained.
        </p>
      ) : null}
      {pending.length ? <p>{pending.length} admitted queries are awaiting a result.</p> : null}
      {receipts.map((receipt) => (
        <details key={receipt.id} className="history">
          <summary>
            {receipt.subject.claim} · {states[receipt.state]} · {receipt.returned.length} candidate
            declarations
          </summary>
          <p>Query: {receipt.query}</p>
          <p>
            Snapshot: <code>{receipt.snapshot ?? 'Not available'}</code>
          </p>
          <p>
            Index: <code>{receipt.index_revision ?? 'Not available'}</code>
          </p>
          {receipt.scopes.map((scope, i) => (
            <p key={i}>Search scope: {scope}</p>
          ))}
          {receipt.limits.map((limit, i) => (
            <p key={i}>{limit}</p>
          ))}
          {receipt.state === 'complete' && receipt.returned.length === 0 ? (
            <p>No candidate was returned within this search scope.</p>
          ) : null}
          {receipt.response ? (
            <p>
              <a
                href={`/api/v1/submissions/${encodeURIComponent(submissionId)}/referee/knowledge/${encodeURIComponent(receipt.id)}`}
              >
                Download the saved source evidence
              </a>{' '}
              · SHA-256 <code>{receipt.response.sha256}</code>
            </p>
          ) : null}
        </details>
      ))}
    </section>
  );
}

export function CoverageEvidence({ coverage }: { coverage: ClaimCoverage }) {
  const binding = coverage.binding;
  return (
    <section aria-label={`Knowledge comparison for ${coverage.subject.claim}`}>
      <h4>{relations[coverage.relation]}</h4>
      <p>
        {coverage.qualification.level === 'source_audited'
          ? 'Source-based reviewer judgment; not a formal coverage certificate.'
          : 'Uncertified proposal; no formal coverage qualification.'}
      </p>
      <Markdown text={coverage.summary} />
      {binding ? (
        <>
          <p>Joint premise set · relation: {binding.relation}</p>
          <ul>
            {binding.premises.map((premise) => (
              <li key={premise.statement_id}>
                {premise.module_path} · {premise.declaration_name_key}
                <br />
                <code>{premise.statement_id}</code>
              </li>
            ))}
          </ul>
          <Markdown text={binding.argument} />
          {binding.parameters.map((parameter, i) => (
            <p key={i}>
              Parameter {parameter.parameter}: {parameter.expression}
            </p>
          ))}
          {binding.hypotheses.map((hypothesis, i) => (
            <div key={i}>
              <Markdown text={`Required hypothesis: ${hypothesis.hypothesis}`} />
              <Markdown text={hypothesis.argument || 'Unresolved hypothesis'} />
            </div>
          ))}
          {binding.unresolved.length ? (
            <>
              <h4>Remaining obligations</h4>
              <ul>
                {binding.unresolved.map((obligation, i) => (
                  <li key={i}>
                    <Markdown text={obligation} />
                  </li>
                ))}
              </ul>
            </>
          ) : null}
        </>
      ) : null}
      {coverage.limits.map((limit, i) => (
        <p key={i}>{limit}</p>
      ))}
      <details>
        <summary>Comparison identity</summary>
        <p>
          Source version {coverage.subject.version} · statements revision{' '}
          {coverage.subject.claims_revision} · round {coverage.subject.round}
        </p>
        <p>Receipts: {coverage.receipt_ids.join(', ')}</p>
      </details>
    </section>
  );
}
