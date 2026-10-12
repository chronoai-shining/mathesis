import { useCallback, useState, type FormEvent } from 'react';
import { useApi } from '../api/context';
import { usePaged } from '../api/usePaged';
import type { LeanDeposit, AttemptView } from '../api/types';
import { useSession } from '../auth/session';
import { Async, Field, InlineError, DateText } from './ui';
import { Pager } from './Pager';

export function LeanDepositForm() {
  const api = useApi();
  const { person } = useSession();
  const [title, setTitle] = useState('');
  const [authors, setAuthors] = useState(person?.display_name ?? '');
  const [statement, setStatement] = useState('');
  const [reference, setReference] = useState('');
  const [toolchain, setToolchain] = useState('');
  const [license, setLicense] = useState('');
  const [file, setFile] = useState<File | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<unknown>(null);
  const [received, setReceived] = useState<LeanDeposit | null>(null);
  const fetchPage = useCallback(
    (before: string | null, signal: AbortSignal) =>
      api.formalizations({ before, limit: 10 }, { signal }),
    [api],
  );
  const deposits = usePaged(fetchPage);
  const submit = async (event: FormEvent) => {
    event.preventDefault();
    setError(null);
    if (!file || !file.name.endsWith('.lean') || file.size > 1048576 || file.size === 0) {
      setError('Choose a nonempty .lean file of at most 1 MB.');
      return;
    }
    setBusy(true);
    try {
      const result = await api.submitFormalization({
        title,
        authors: authors
          .split('\n')
          .map((a) => a.trim())
          .filter(Boolean),
        statement,
        source_reference: { kind: 'url', locator: reference },
        toolchain,
        license,
        solution: await file.text(),
      });
      setReceived(result);
      deposits.reload();
    } catch (e) {
      setError(e);
    } finally {
      setBusy(false);
    }
  };
  return (
    <>
      <p className="lede">
        Submit an existing Lean formalization, with its original authors and research source.
      </p>
      <p className="hint">
        We preserve your source and attribution. To verify it, link an accepted statement and submit
        a module importing its confirmed Target.lean. Only an independent verifier receipt earns
        Lean ✓. This intake accepts one Lean module; arbitrary Lake projects and external
        dependencies are not built here.
      </p>
      <form
        className="form lean-deposit-form"
        aria-label="Submit Lean formalization"
        onSubmit={(e) => {
          void submit(e);
        }}
      >
        <Field label="Title" htmlFor="lean-title">
          <input
            id="lean-title"
            required
            maxLength={1000}
            value={title}
            onChange={(e) => setTitle(e.target.value)}
          />
        </Field>
        <Field label="Original authors (one per line)" htmlFor="lean-authors">
          <textarea
            id="lean-authors"
            required
            value={authors}
            onChange={(e) => setAuthors(e.target.value)}
          />
        </Field>
        <Field label="Mathematical statement" htmlFor="lean-statement">
          <textarea
            id="lean-statement"
            required
            maxLength={20000}
            rows={5}
            value={statement}
            onChange={(e) => setStatement(e.target.value)}
          />
        </Field>
        <Field label="Original research or source URL" htmlFor="lean-reference">
          <input
            id="lean-reference"
            type="url"
            required
            placeholder="Link to the paper or .lean source at a fixed revision"
            value={reference}
            onChange={(e) => setReference(e.target.value)}
          />
        </Field>
        <Field label="Lean / Mathlib version pin" htmlFor="lean-toolchain">
          <input
            id="lean-toolchain"
            required
            maxLength={200}
            placeholder="leanprover/lean4:v4.x, Mathlib <revision>"
            value={toolchain}
            onChange={(e) => setToolchain(e.target.value)}
          />
        </Field>
        <Field label="Source license" htmlFor="lean-license">
          <input
            id="lean-license"
            required
            maxLength={200}
            placeholder="e.g. Apache-2.0"
            value={license}
            onChange={(e) => setLicense(e.target.value)}
          />
        </Field>
        <Field label="Lean source (up to 1 MB)" htmlFor="lean-source">
          <input
            id="lean-source"
            type="file"
            accept=".lean"
            required
            onChange={(e) => setFile(e.target.files?.[0] ?? null)}
          />
        </Field>
        <InlineError error={error} />
        <button className="button" type="submit" disabled={busy}>
          {busy ? 'Saving source…' : 'Submit formalization'}
        </button>
      </form>
      {received ? (
        <p className="notice notice-info" role="status">
          Source received: {received.title}. Verification has not run yet.
        </p>
      ) : null}
      <section>
        <h2>Your formalization submissions</h2>
        <Async state={deposits.state} onRetry={deposits.reload}>
          {(page) =>
            page.items.length ? (
              <ul className="entry-list">
                {page.items.map((d) => (
                  <li key={d.id}>
                    <DepositEntry deposit={d} onChange={deposits.reload} />
                  </li>
                ))}
              </ul>
            ) : (
              <p className="muted">Your submitted Lean sources will appear here.</p>
            )
          }
        </Async>
        <Pager paged={deposits} />
      </section>
    </>
  );
}
function DepositEntry({ deposit, onChange }: { deposit: LeanDeposit; onChange: () => void }) {
  const api = useApi();
  const [record, setRecord] = useState('');
  const [claim, setClaim] = useState('');
  const [result, setResult] = useState<AttemptView | null>(null);
  const [error, setError] = useState<unknown>(null);
  const [busy, setBusy] = useState(false);
  const run = async (event: FormEvent) => {
    event.preventDefault();
    setError(null);
    setBusy(true);
    try {
      setResult(await api.verifyFormalization(deposit.id, record.trim(), claim.trim()));
      onChange();
    } catch (e) {
      setError(e);
    } finally {
      setBusy(false);
    }
  };
  const refresh = async (id: string) => {
    try {
      setResult(await api.getAttempt(id));
    } catch (e) {
      setError(e);
    }
  };
  return (
    <article>
      <h3>{deposit.title}</h3>
      <p className="small muted">
        {deposit.authors.join(', ')} · received <DateText iso={deposit.submitted_at} />
      </p>
      <p>
        <a href={`/api/v1/formalizations/${encodeURIComponent(deposit.id)}/source`}>
          Download submitted source
        </a>{' '}
        · {deposit.license}
      </p>
      <details>
        <summary>Verify against a confirmed statement</summary>
        <p className="hint">
          Use a source module that imports Target and declares wishpool_solution or
          wishpool_disproof. The stored toolchain must match the target. Verified proofs and their
          original attribution appear on the public source record.
        </p>
        <form
          className="form"
          onSubmit={(e) => {
            void run(e);
          }}
        >
          <label>
            Paper record
            <input
              required
              placeholder="WP-2026-0001"
              value={record}
              onChange={(e) => setRecord(e.target.value)}
            />
          </label>
          <label>
            Statement
            <input
              required
              placeholder="C1"
              value={claim}
              onChange={(e) => setClaim(e.target.value)}
            />
          </label>
          <button className="button button-small" disabled={busy}>
            {busy ? 'Submitting…' : 'Request verification'}
          </button>
        </form>
      </details>
      {deposit.attempts.map((id) => (
        <button
          key={id}
          type="button"
          className="button button-quiet button-small"
          onClick={() => {
            void refresh(id);
          }}
        >
          Check verification {id.slice(0, 8)}
        </button>
      ))}
      {result ? (
        <p role="status">
          Verification: <strong>{result.state}</strong>
          {result.receipt
            ? ` — ${result.receipt.reason}`
            : '. Waiting for an independent verifier receipt.'}
        </p>
      ) : null}
      <InlineError error={error} />
    </article>
  );
}
