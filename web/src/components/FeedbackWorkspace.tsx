import { useCallback, useState, type FormEvent } from 'react';
import { useApi } from '../api/context';
import { useAsync } from '../api/useAsync';
import { useSession } from '../auth/session';
import type {
  FeedbackKind,
  FeedbackPreview,
  Submission,
  KnowledgeDeclarationRef,
} from '../api/types';
import { Async, Field, InlineError } from './ui';
import { Link } from '../routing/router';

export function FeedbackWorkspace({ submission }: { submission: Submission }) {
  const api = useApi();
  const { person } = useSession();
  const load = useCallback(
    (signal: AbortSignal) => api.feedbacks(submission.id, { signal }),
    [api, submission.id],
  );
  const resource = useAsync(load);
  return (
    <details className="report">
      <summary>Authorized knowledge proposals</summary>
      <p>
        Prepare a proposal from accepted public work for Trureturing’s existing intake. Every
        required rights holder approves the exact package separately before download. Upstream
        admission has its own review.
      </p>
      {person?.id === submission.submitter ? (
        <PrepareFeedback submission={submission} changed={resource.reload} />
      ) : null}
      <button className="button button-quiet" onClick={resource.reload}>
        Refresh proposals
      </button>
      <Async state={resource.state} onRetry={resource.reload}>
        {(items) => (
          <>
            {!items.length ? <p>No proposals prepared.</p> : null}
            {items.map((p) => (
              <p key={p.id}>
                <Link to={{ kind: 'feedback', id: submission.id, key: p.id }}>
                  {p.kind.kind.replaceAll('_', ' ')} · {p.source.record}
                </Link>{' '}
                —{' '}
                {p.events.some((e) => e.event === 'revoked')
                  ? 'Revoked'
                  : 'Open authorization and observation history'}
              </p>
            ))}
          </>
        )}
      </Async>
    </details>
  );
}
function PrepareFeedback({ submission, changed }: { submission: Submission; changed: () => void }) {
  const api = useApi();
  const [kind, setKind] = useState<FeedbackKind['kind']>('open_problem');
  const [preview, setPreview] = useState<FeedbackPreview>();
  const [consent, setConsent] = useState(false);
  const [error, setError] = useState<unknown>();
  const [busy, setBusy] = useState(false);
  async function inspect(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const data = new FormData(event.currentTarget);
    const value = (key: string) => String(data.get(key) ?? '');
    setBusy(true);
    setError(undefined);
    setConsent(false);
    setPreview(undefined);
    try {
      let input: FeedbackKind;
      switch (kind) {
        case 'open_problem':
          input = { kind, claim: value('claim') };
          break;
        case 'verified_solution':
          input = { kind, claim: value('claim'), attempt: value('attempt') };
          break;
        case 'dtr_registration':
          input = {
            kind,
            claim: value('claim'),
            attempt: value('attempt'),
            slots: {
              escape_from: value('escape_from'),
              readout_via: value('readout_via'),
              realization: value('realization'),
              escape_continues: value('escape_continues'),
            },
          };
          break;
        case 'library_binding':
          input = { kind, board: value('board'), contribution: value('contribution') };
          break;
        case 'witness_formalization':
          input = { kind, claim: value('claim'), witness: Number(value('witness')) };
          break;
        case 'erratum':
          input = {
            kind,
            report: value('report'),
            upstream: JSON.parse(value('upstream')) as KnowledgeDeclarationRef[],
          };
          break;
      }
      setPreview(await api.previewFeedback(submission.id, input));
    } catch (e) {
      setError(e);
    } finally {
      setBusy(false);
    }
  }
  async function prepare() {
    if (!preview || !consent) return;
    setBusy(true);
    setError(undefined);
    try {
      await api.prepareFeedback(submission.id, {
        kind: preview.kind,
        preview_digest: preview.digest,
        preparation_consent: 'feedback-prepare-2026-10-16',
      });
      setPreview(undefined);
      setConsent(false);
      changed();
    } catch (e) {
      setError(e);
    } finally {
      setBusy(false);
    }
  }
  return (
    <section>
      <h3>Prepare an exact proposal</h3>
      <form onSubmit={(e) => void inspect(e)}>
        <fieldset
          disabled={busy}
          onChange={() => {
            setPreview(undefined);
            setConsent(false);
          }}
        >
          <legend>Proposal evidence</legend>
          <Field label="Proposal channel" htmlFor="feedback-kind">
            <select
              id="feedback-kind"
              value={kind}
              onChange={(e) => setKind(e.target.value as FeedbackKind['kind'])}
            >
              <option value="open_problem">Open problem</option>
              <option value="verified_solution">Verified solution</option>
              <option value="library_binding">Library binding</option>
              <option value="witness_formalization">Content witness for formalization</option>
              <option value="dtr_registration">DTR registration proposal</option>
              <option value="erratum">Confirmed erratum</option>
            </select>
          </Field>
          {kind !== 'library_binding' && kind !== 'erratum' ? (
            <Field label="Confirmed statement" htmlFor="feedback-claim">
              <select name="claim" id="feedback-claim">
                {submission.claims.map((c) => (
                  <option key={c.id} value={c.id}>
                    {c.id}: {c.statement.slice(0, 150)}
                  </option>
                ))}
              </select>
            </Field>
          ) : null}
          {kind === 'verified_solution' || kind === 'dtr_registration' ? (
            <Text name="attempt" label="Verified attempt ID" />
          ) : null}
          {kind === 'library_binding' ? (
            <>
              <Text name="board" label="Public binding summary ID" />
              <Text name="contribution" label="Supported contribution ID" />
            </>
          ) : null}
          {kind === 'witness_formalization' ? (
            <Field label="Witness number (first witness is 0)" htmlFor="feedback-witness">
              <input
                type="number"
                id="feedback-witness"
                name="witness"
                required
                min={0}
                defaultValue={0}
              />
            </Field>
          ) : null}
          {kind === 'erratum' ? (
            <>
              <Text name="report" label="Confirmed error report ID" />
              <Field
                label="Affected upstream declaration identities (JSON array)"
                htmlFor="feedback-upstream"
              >
                <textarea id="feedback-upstream" name="upstream" required rows={8} />
              </Field>
            </>
          ) : null}
          {kind === 'dtr_registration' ? (
            <>
              <p>
                Describe all four proposed registration slots. Upstream checks determine whether a
                registration can be admitted.
              </p>
              {(['escape_from', 'readout_via', 'realization', 'escape_continues'] as const).map(
                (name) => (
                  <Field key={name} label={name.replaceAll('_', ' ')} htmlFor={`feedback-${name}`}>
                    <textarea id={`feedback-${name}`} name={name} required maxLength={5000} />
                  </Field>
                ),
              )}
            </>
          ) : null}
        </fieldset>
        <button className="button" disabled={busy}>
          Preview exact evidence
        </button>
      </form>
      <InlineError error={error} />
      {preview ? (
        <>
          <h4>Exact package for approval</h4>
          <p>Required rights holders: {preview.required_signers.join(', ')}</p>
          <p>
            Payload digest: <code>{preview.digest}</code>
          </p>
          <pre className="code-block">{JSON.stringify(preview.payload, null, 2)}</pre>
          <label>
            <input
              type="checkbox"
              checked={consent}
              onChange={(e) => setConsent(e.target.checked)}
            />
            I approve preparing this exact package for the listed rights holders to inspect. Each
            holder must separately authorize export under Apache-2.0.
          </label>
          <button className="button" disabled={busy || !consent} onClick={() => void prepare()}>
            Save proposal for authorization
          </button>
        </>
      ) : null}
    </section>
  );
}
function Text({ name, label }: { name: string; label: string }) {
  return (
    <Field label={label} htmlFor={`feedback-${name}`}>
      <input id={`feedback-${name}`} name={name} required maxLength={200} />
    </Field>
  );
}
