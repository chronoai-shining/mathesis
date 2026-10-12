import { ProofAttribution } from '../components/ProofAttribution';
import { useCallback } from 'react';
import { useApi } from '../api/context';
import { useAsync } from '../api/useAsync';
import { Async } from '../components/ui';
import { LatexText, MacrosProvider } from '../components/Markdown';
import { ReportError, ErrorReports } from '../components/ErrorReports';
import { AttemptForm } from './Conjecture';
import { EntrantName } from './Leaderboard';
import { Link } from '../routing/router';

export function SupportersPage() {
  const api = useApi();
  const load = useCallback((signal: AbortSignal) => api.supporters({ signal }), [api]);
  const supporters = useAsync(load);
  return (
    <div className="page narrow-page">
      <header className="page-head">
        <div>
          <h1>Supporters</h1>
          <p className="lede">People who support our agents’ work.</p>
          <p>
            Tokens count only when NyxID reports usage for a funded run. Names are listed
            alphabetically.
          </p>
        </div>
      </header>
      <Async state={supporters.state} onRetry={supporters.reload}>
        {(items) =>
          items.length ? (
            <ul className="entry-list supporter-list">
              {items.map((s, i) => (
                <li key={`${s.name}-${i}`}>
                  <strong>{s.name}</strong>
                  <span>{s.tokens.toLocaleString()} metered tokens</span>
                </li>
              ))}
            </ul>
          ) : (
            <p className="muted">No supporters yet.</p>
          )
        }
      </Async>
      <Link to={{ kind: 'contribute' }}>Donate quota</Link>
    </div>
  );
}
export function StatementPage({ record, claim }: { record: string; claim: string }) {
  const api = useApi();
  const load = useCallback(
    (signal: AbortSignal) => api.getStatement(record, claim, { signal }),
    [api, record, claim],
  );
  const statement = useAsync(load);
  return (
    <div className="page">
      <Async state={statement.state} onRetry={statement.reload}>
        {(s) => (
          <MacrosProvider macros={s.macros}>
            <article>
              <p className="eyebrow">
                <Link to={{ kind: 'paper', record }}>{record}</Link> · {claim}
              </p>
              <h1>{s.claim.label}</h1>
              <p className="small muted">Reviewed by GPT Pro and audited by Codex</p>
              <LatexText source={s.claim.statement} />
              <ReportError record={record} subject={{ kind: 'statement', claim }} />
              {s.claim.verified ? (
                <p className="notice">
                  <strong>Lean ✓</strong> · <EntrantName entrant={s.claim.verified.entrant} />{' '}
                  proved the author-confirmed Lean target.
                </p>
              ) : null}
              <section>
                <h2>Lean target</h2>
                {s.target ? (
                  <>
                    <p>The author confirmed this exact target.</p>
                    <details>
                      <summary>View Target.lean</summary>
                      <pre>
                        <code>{s.target.lean}</code>
                      </pre>
                    </details>
                    <a
                      className="button button-quiet"
                      href={`/api/v1/papers/${encodeURIComponent(record)}/statements/${encodeURIComponent(claim)}/target`}
                      download="Target.lean"
                    >
                      Download Target.lean
                    </a>
                    <ReportError
                      record={record}
                      subject={{ kind: 'target', claim, digest: s.target.digest }}
                    />
                  </>
                ) : (
                  <p className="muted">
                    A target becomes available after author approval, our faithfulness check, and
                    author confirmation.
                  </p>
                )}
              </section>
              {s.target ? (
                <section>
                  <h2>Submit an attempt</h2>
                  <p>
                    Import Target and prove wishpool_solution : wishpool_target_prop, or
                    wishpool_disproof : ¬ wishpool_target_prop.
                  </p>
                  <AttemptForm record={record} claim={claim} statement />
                </section>
              ) : null}
              {s.verified_attempts.length ? (
                <section>
                  <h2>Verified attempts</h2>
                  <ul className="entry-list">
                    {s.verified_attempts.map((a) => (
                      <li key={a.id}>
                        <EntrantName entrant={a.entrant} />
                        <ProofAttribution attribution={a.attribution} />
                        <p>
                          {a.receipt.verdict === 'proved' ? 'Proved' : 'Disproved'} the
                          author-confirmed Lean target.
                        </p>
                        <details>
                          <summary>Solution and receipt</summary>
                          <pre>{a.solution}</pre>
                          <pre>{JSON.stringify(a.receipt, null, 2)}</pre>
                        </details>
                      </li>
                    ))}
                  </ul>
                </section>
              ) : null}
              <ErrorReports record={record} />
            </article>
          </MacrosProvider>
        )}
      </Async>
    </div>
  );
}
