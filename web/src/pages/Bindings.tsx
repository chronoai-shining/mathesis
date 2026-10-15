import { useCallback, useState, type FormEvent } from 'react';
import { useApi } from '../api/context';
import { useAsync } from '../api/useAsync';
import { useSession } from '../auth/session';
import type { KnowledgeBinding, PublicBindingBoard } from '../api/types';
import { Async, Field, InlineError, DateText } from '../components/ui';
import { Markdown } from '../components/Markdown';
import { Link } from '../routing/router';

export function BindingsPage() {
  const api = useApi();
  const [before, setBefore] = useState<string>();
  const load = useCallback(
    (signal: AbortSignal) => api.bindings(before, { signal }),
    [api, before],
  );
  const resource = useAsync(load);
  return (
    <div className="page">
      <header className="page-head">
        <div>
          <h1>Binding discussions</h1>
          <p className="lede">
            Connect author-approved statements to existing mathematics. Submit a joint argument,
            inspect a contribution, or bring evidence to a challenge.
          </p>
        </div>
      </header>
      <Async state={resource.state} onRetry={resource.reload}>
        {(page) => (
          <>
            {!page.items.length ? <p>No authorized summaries on this page.</p> : null}
            {page.items.map((b) => (
              <article className="panel" key={b.id}>
                <Link to={{ kind: 'binding', id: b.submission, key: b.id }}>
                  <h2>{b.claim}</h2>
                </Link>
                <Markdown text={b.statement} />
                <p>
                  {b.contributions} contributions · {b.credits} credited bindings
                </p>
              </article>
            ))}
            {page.next_before ? (
              <button
                className="button button-quiet"
                onClick={() => setBefore(page.next_before ?? undefined)}
              >
                Next summaries
              </button>
            ) : null}
          </>
        )}
      </Async>
    </div>
  );
}
export function BindingPage({ id, boardKey }: { id: string; boardKey: string }) {
  const api = useApi();
  const { person } = useSession();
  const load = useCallback(
    (signal: AbortSignal) => api.binding(id, boardKey, { signal }),
    [api, id, boardKey],
  );
  const resource = useAsync(load);
  return (
    <div className="page">
      <header className="page-head">
        <div>
          <h1>Binding discussion</h1>
          <p>
            <Link to={{ kind: 'bindings' }}>All binding discussions</Link>
          </p>
        </div>
      </header>
      <button className="button button-quiet" onClick={resource.reload}>
        Refresh discussion
      </button>
      <Async state={resource.state} onRetry={resource.reload}>
        {(b) => (
          <>
            <section className="panel">
              <h2>Author-approved statement</h2>
              <Markdown text={b.target.statement} />
              <Markdown text={b.context} />
              <p>
                Shared by {b.authorized_by} · <DateText iso={b.authorized_at} /> · {b.license}
              </p>
              <p>
                Source version {b.target.subject.version}; statements revision{' '}
                {b.target.subject.claims_revision}. Only independently supported equivalence,
                instance or implication earns binding-discovery credit. Proof use and refutation
                remain visible with their own meaning; this credit is separate from verified proofs
                and publication.
              </p>
              <h3>Approved source declarations</h3>
              <ul>
                {b.target.sources.map((s) => (
                  <li key={s.statement_id}>
                    <strong>{s.declaration_name_key}</strong> — {s.module_path}
                    <details>
                      <summary>Exact source identity</summary>
                      <pre className="code-block">{JSON.stringify(s, null, 2)}</pre>
                    </details>
                  </li>
                ))}
              </ul>
            </section>
            <section>
              <h2>Current binding credit</h2>
              {b.credits.length ? (
                <ul>
                  {b.credits.map((c) => (
                    <li key={c.canonical}>
                      <Link to={{ kind: 'binding_profile', person: c.contributor }}>
                        {c.contributor}
                      </Link>{' '}
                      —{' '}
                      <a href={`#contribution-${c.contribution}`}>
                        independently supported binding
                      </a>
                    </li>
                  ))}
                </ul>
              ) : (
                <p>No currently supported binding has earned credit.</p>
              )}
            </section>
            {b.contributions.map((c) => (
              <section className="panel" id={`contribution-${c.id}`} key={c.id}>
                <h2>
                  Contribution by{' '}
                  <Link to={{ kind: 'binding_profile', person: c.contributor }}>
                    {c.contributor}
                  </Link>
                </h2>
                <p>
                  {c.binding.relation} · {c.license}
                </p>
                <Markdown text={c.binding.argument} />
                <details>
                  <summary>Joint premises and correspondence</summary>
                  <pre className="code-block">{JSON.stringify(c.binding, null, 2)}</pre>
                </details>
                <h3>Independent inspection history</h3>
                {b.audits
                  .filter((a) => a.contribution === c.id)
                  .map((a) => (
                    <p key={a.request}>
                      {a.verdict} ·{' '}
                      {a.evidence_complete
                        ? 'Required source excerpts present'
                        : 'Incomplete source evidence'}{' '}
                      · <DateText iso={a.at} />
                    </p>
                  ))}
                {b.challenges
                  .filter((a) => a.contribution === c.id)
                  .map((a) => (
                    <article key={a.id}>
                      <h3>Challenge or appeal by {a.challenger}</h3>
                      <Markdown text={a.evidence} />
                      <p>
                        {a.license} · <DateText iso={a.at} />
                      </p>
                    </article>
                  ))}
                {person ? (
                  <ChallengeForm board={b} contribution={c.id} changed={resource.replace} />
                ) : null}
              </section>
            ))}
            {person ? (
              <ContributionForm board={b} changed={resource.replace} />
            ) : (
              <p>Sign in to contribute or challenge a binding.</p>
            )}
          </>
        )}
      </Async>
    </div>
  );
}
const lines = (value: FormDataEntryValue | null) =>
  String(value ?? '')
    .split('\n')
    .map((s) => s.trim())
    .filter(Boolean);
function pairs(value: FormDataEntryValue | null): [string, string][] {
  return lines(value).map((line) => {
    const i = line.indexOf('=>');
    if (i < 1) throw new Error('Use one “name => correspondence” pair per line.');
    return [line.slice(0, i).trim(), line.slice(i + 2).trim()];
  });
}
function ContributionForm({
  board: b,
  changed,
}: {
  board: PublicBindingBoard;
  changed: (b: PublicBindingBoard) => void;
}) {
  const api = useApi();
  const [error, setError] = useState<unknown>();
  const [busy, setBusy] = useState(false);
  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const form = event.currentTarget;
    const data = new FormData(form);
    if (data.get('consent') !== 'yes') return;
    setBusy(true);
    setError(undefined);
    try {
      const binding: KnowledgeBinding = {
        premises: b.target.sources.filter((s) => data.getAll('premise').includes(s.statement_id)),
        relation: String(data.get('relation')) as KnowledgeBinding['relation'],
        parameters: pairs(data.get('parameters')).map(([parameter, expression]) => ({
          parameter,
          expression,
        })),
        hypotheses: pairs(data.get('hypotheses')).map(([hypothesis, argument]) => ({
          hypothesis,
          argument,
        })),
        argument: String(data.get('argument')),
        unresolved: lines(data.get('unresolved')),
        source_lineage: lines(data.get('lineage')),
      };
      changed(
        await api.contributeBinding(b.target.subject.submission, b.id, {
          binding,
          license: 'CC-BY-4.0',
          consent: 'binding-contribution-2026-10-16',
        }),
      );
      form.reset();
    } catch (e) {
      setError(e);
    } finally {
      setBusy(false);
    }
  }
  return (
    <section className="panel">
      <h2>Contribute a joint binding</h2>
      <form onSubmit={(e) => void submit(e)}>
        <fieldset>
          <legend>Premises used together</legend>
          {b.target.sources.map((s) => (
            <label key={s.statement_id}>
              <input type="checkbox" name="premise" value={s.statement_id} />
              {s.declaration_name_key}
            </label>
          ))}
        </fieldset>
        <Field label="Relation" htmlFor={`relation-${b.id}`}>
          <select id={`relation-${b.id}`} name="relation">
            <option value="implies">Joint premises imply the statement</option>
            <option value="equivalent">Equivalent</option>
            <option value="instance">Statement is an instance</option>
            <option value="refutes">Refutes the statement</option>
            <option value="proof_use">Used in the proof</option>
          </select>
        </Field>
        <Field
          label="Parameter substitutions (one parameter => expression per line)"
          htmlFor={`parameters-${b.id}`}
        >
          <textarea id={`parameters-${b.id}`} name="parameters" maxLength={20000} />
        </Field>
        <Field
          label="Hypothesis discharge (one hypothesis => argument per line)"
          htmlFor={`hypotheses-${b.id}`}
        >
          <textarea id={`hypotheses-${b.id}`} name="hypotheses" maxLength={20000} />
        </Field>
        <Field label="Joint mathematical argument" htmlFor={`argument-${b.id}`}>
          <textarea id={`argument-${b.id}`} name="argument" required maxLength={20000} rows={8} />
        </Field>
        <Field
          label="Remaining obligations (one per line; leave empty only if none)"
          htmlFor={`unresolved-${b.id}`}
        >
          <textarea id={`unresolved-${b.id}`} name="unresolved" maxLength={20000} />
        </Field>
        <Field
          label="Known source Mathesis submission IDs (one per line, if any)"
          htmlFor={`lineage-${b.id}`}
        >
          <textarea id={`lineage-${b.id}`} name="lineage" maxLength={10000} />
        </Field>
        <ContributionConsent />
        <InlineError error={error} />
        <button className="button" disabled={busy}>
          Submit for independent inspection
        </button>
      </form>
    </section>
  );
}
function ContributionConsent() {
  return (
    <label>
      <input type="checkbox" name="consent" value="yes" required />I have the right to publish this
      contribution, license it under CC BY 4.0 with attribution, and agree to independent AI
      inspection. I understand that submission does not establish correctness or earn credit.
    </label>
  );
}
function ChallengeForm({
  board: b,
  contribution,
  changed,
}: {
  board: PublicBindingBoard;
  contribution: string;
  changed: (b: PublicBindingBoard) => void;
}) {
  const api = useApi();
  const [error, setError] = useState<unknown>();
  const [busy, setBusy] = useState(false);
  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const form = event.currentTarget;
    const data = new FormData(form);
    if (data.get('consent') !== 'yes') return;
    setBusy(true);
    setError(undefined);
    try {
      changed(
        await api.challengeBinding(b.target.subject.submission, b.id, {
          contribution,
          evidence: String(data.get('evidence')),
          license: 'CC-BY-4.0',
          consent: 'binding-contribution-2026-10-16',
        }),
      );
      form.reset();
    } catch (e) {
      setError(e);
    } finally {
      setBusy(false);
    }
  }
  return (
    <details>
      <summary>Challenge this binding or appeal its inspection</summary>
      <form onSubmit={(e) => void submit(e)}>
        <Field
          label="Reproducible challenge or appeal evidence"
          htmlFor={`challenge-${contribution}`}
        >
          <textarea
            id={`challenge-${contribution}`}
            name="evidence"
            required
            maxLength={20000}
            rows={6}
          />
        </Field>
        <ContributionConsent />
        <InlineError error={error} />
        <button className="button button-quiet" disabled={busy}>
          Request independent reinspection
        </button>
      </form>
    </details>
  );
}

export function BindingProfilePage({ person }: { person: string }) {
  const api = useApi();
  const [before, setBefore] = useState<string>();
  const load = useCallback(
    (signal: AbortSignal) => api.bindingProfile(person, before, { signal }),
    [api, person, before],
  );
  const resource = useAsync(load);
  return (
    <div className="page">
      <header className="page-head">
        <div>
          <h1>Binding contributions</h1>
          <p>{person}</p>
          <p>
            Current public work, with one credit per independently supported joint binding.
            Historical decisions remain on each discussion.
          </p>
        </div>
      </header>
      <Async state={resource.state} onRetry={resource.reload}>
        {(page) => (
          <>
            {page.items.length ? (
              page.items.map((e) => (
                <article className="panel" key={e.contribution.id}>
                  <Markdown text={e.statement} />
                  <p>
                    {e.credited ? 'Credited binding' : (e.latest_verdict ?? 'Awaiting inspection')}
                  </p>
                  <Link to={{ kind: 'binding', id: e.submission, key: e.board }}>
                    Open contribution and audit history
                  </Link>
                </article>
              ))
            ) : (
              <p>No currently public binding contributions on this page.</p>
            )}
            {page.next_before ? (
              <button
                className="button button-quiet"
                onClick={() => setBefore(page.next_before ?? undefined)}
              >
                Next contributions
              </button>
            ) : null}
          </>
        )}
      </Async>
    </div>
  );
}
