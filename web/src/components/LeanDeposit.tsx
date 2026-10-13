import { useCallback, useState, type FormEvent } from 'react';
import { useApi } from '../api/context';
import { useAsync } from '../api/useAsync';
import { Link } from '../routing/router';
import { usePaged, type Paged } from '../api/usePaged';
import type { LeanDeposit, AttemptView, StatementDetail } from '../api/types';
import { useSession } from '../auth/session';
import { Async, Field, InlineError, DateText } from './ui';
import { Pager } from './Pager';

export function LeanDepositForm({
  targetRecord,
  targetClaim,
}: {
  targetRecord?: string;
  targetClaim?: string;
}) {
  const api = useApi();
  const load = useCallback(
    async (signal: AbortSignal) => {
      if (!targetRecord || !targetClaim) return null;
      const [statement, paper] = await Promise.all([
        api.getStatement(targetRecord, targetClaim, { signal }),
        api.getPaper(targetRecord, { signal }),
      ]);
      if (!statement.target)
        throw new Error(
          'This statement no longer has a confirmed target. Choose a current statement.',
        );
      return { statement, authors: paper.summary.authors.map((a) => a.name) };
    },
    [api, targetRecord, targetClaim],
  );
  const context = useAsync(load);
  return (
    <Async state={context.state} onRetry={context.reload}>
      {(data) => (
        <DepositForm
          key={data?.statement.target?.digest ?? 'standalone'}
          target={data?.statement}
          originalAuthors={data?.authors}
        />
      )}
    </Async>
  );
}
function DepositForm({
  target,
  originalAuthors,
}: {
  target?: StatementDetail;
  originalAuthors?: string[];
}) {
  const api = useApi();
  const { person } = useSession();
  const [title, setTitle] = useState(target ? `${target.title} — ${target.claim.label}` : '');
  const [authors, setAuthors] = useState(originalAuthors?.join('\n') ?? person?.display_name ?? '');
  const [statement, setStatement] = useState(target?.claim.statement ?? '');
  const [reference, setReference] = useState(
    target ? `${window.location.origin}/papers/${encodeURIComponent(target.record)}` : '',
  );
  const [toolchain, setToolchain] = useState(target?.target?.toolchain ?? '');
  const [formalizers, setFormalizers] = useState(person?.display_name ?? '');
  const [sourceRevision, setSourceRevision] = useState('');
  const [supporting, setSupporting] = useState<File[]>([]);
  const [verification, setVerification] = useState<AttemptView | null>(null);
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
      const modules: Record<string, string> = {};
      if (
        supporting.length > 32 ||
        supporting.reduce((size, f) => size + f.size, file.size) > 1048576
      )
        throw new Error('Use at most 32 supporting files and 1 MB of total Lean source.');
      for (const module of supporting) {
        const name = module.name.replace(/\.lean$/, '');
        if (
          !module.name.endsWith('.lean') ||
          !/^[A-Z][A-Za-z0-9_]{0,63}$/.test(name) ||
          ['target', 'solution', 'check', 'mathlib', 'init', 'lean', 'std', 'lake'].includes(
            name.toLowerCase(),
          ) ||
          Object.keys(modules).some((m) => m.toLowerCase() === name.toLowerCase())
        )
          throw new Error(
            'Supporting files need distinct capitalized module names, such as Lemmas.lean. Do not upload Target.lean as a supporting file.',
          );
        modules[name] = await module.text();
      }
      const result = await api.submitFormalization({
        ...(supporting.length ? { modules } : {}),
        formalizers: formalizers
          .split('\n')
          .map((a) => a.trim())
          .filter(Boolean),
        ...(sourceRevision.trim() ? { source_revision: sourceRevision.trim() } : {}),
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
      if (target?.target) {
        setVerification(
          await api.verifyFormalization(
            result.id,
            target.record,
            target.claim.id,
            target.target.digest,
          ),
        );
        deposits.reload();
      }
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
      {target?.target ? (
        <div className="notice notice-info">
          <strong>
            {target.claim.label} · {target.title}
          </strong>
          <p>
            The author confirmed this exact target. Your submission will request independent
            verification against it.
          </p>
          <a
            href={`/api/v1/papers/${encodeURIComponent(target.record)}/statements/${encodeURIComponent(target.claim.id)}/target`}
            download="Target.lean"
          >
            Download Target.lean
          </a>
          <p className="small">Toolchain: {target.target.toolchain}</p>
        </div>
      ) : (
        <p className="hint">
          Your source stays in your private workspace until a linked proof is independently
          verified. <Link to={{ kind: 'formalizations' }}>Choose a confirmed statement</Link> to
          submit directly for verification.
        </p>
      )}
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
        <Field label="Formalization contributors (one per line)" htmlFor="lean-formalizers">
          <textarea
            id="lean-formalizers"
            value={formalizers}
            onChange={(e) => setFormalizers(e.target.value)}
            placeholder="People who wrote the Lean formalization"
          />
        </Field>
        <Field
          label="Git commit (optional)"
          htmlFor="lean-revision"
          hint="Full 40 or 64 character commit hash for the reported source."
        >
          <input
            id="lean-revision"
            maxLength={64}
            pattern="[a-fA-F0-9]{40}|[a-fA-F0-9]{64}"
            value={sourceRevision}
            onChange={(e) => setSourceRevision(e.target.value)}
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
        <Field
          label="Supporting Lean modules (optional)"
          htmlFor="lean-modules"
          hint="Up to 32 flat .lean files, 1 MB total with the main file. Use imports such as import Lemmas. Mathlib comes from the confirmed toolchain; custom dependencies and Lake scripts are not executed."
        >
          <input
            id="lean-modules"
            type="file"
            accept=".lean"
            multiple
            onChange={(e) => setSupporting(Array.from(e.target.files ?? []))}
          />
        </Field>
        <InlineError error={error} />
        <button className="button" type="submit" disabled={busy}>
          {busy
            ? 'Submitting…'
            : target
              ? 'Submit and request verification'
              : 'Submit formalization'}
        </button>
      </form>
      {received ? (
        <p className="notice notice-info" role="status">
          Source received: {received.title}.{' '}
          {verification ? `Verification ${verification.state}.` : 'Verification has not run yet.'}
        </p>
      ) : null}
      <DepositListing deposits={deposits} target={target} />
    </>
  );
}
export function LeanWorkspace() {
  const api = useApi();
  const fetchPage = useCallback(
    (before: string | null, signal: AbortSignal) =>
      api.formalizations({ before, limit: 10 }, { signal }),
    [api],
  );
  const deposits = usePaged(fetchPage);
  return <DepositListing deposits={deposits} />;
}
function DepositListing({
  deposits,
  target,
}: {
  deposits: Paged<LeanDeposit>;
  target?: StatementDetail;
}) {
  return (
    <section>
      <h2>Your formalization submissions</h2>
      <Async state={deposits.state} onRetry={deposits.reload}>
        {(page) =>
          page.items.length ? (
            <ul className="entry-list">
              {page.items.map((d) => (
                <li key={d.id}>
                  <DepositEntry deposit={d} onChange={deposits.reload} target={target} />
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
  );
}
function DepositEntry({
  deposit,
  onChange,
  target,
}: {
  deposit: LeanDeposit;
  onChange: () => void;
  target?: StatementDetail;
}) {
  const api = useApi();
  const [record, setRecord] = useState(target?.record ?? '');
  const [claim, setClaim] = useState(target?.claim.id ?? '');
  const [result, setResult] = useState<AttemptView | null>(null);
  const [error, setError] = useState<unknown>(null);
  const [busy, setBusy] = useState(false);
  const run = async (event: FormEvent) => {
    event.preventDefault();
    setError(null);
    setBusy(true);
    try {
      const current = await api.getStatement(record.trim(), claim.trim());
      if (!current.target) throw new Error('The author has not confirmed a target.');
      if (target?.target && current.target.digest !== target.target.digest)
        throw new Error('Target changed. Reopen the statement before verification.');
      setResult(
        await api.verifyFormalization(
          deposit.id,
          record.trim(),
          claim.trim(),
          current.target.digest,
        ),
      );
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
        · {deposit.license} ·{' '}
        <a href={`/api/v1/formalizations/${encodeURIComponent(deposit.id)}/project`}>
          Download complete project
        </a>
        {deposit.modules && Object.keys(deposit.modules).length
          ? ` · ${Object.keys(deposit.modules).length} supporting modules`
          : ''}
      </p>
      <details>
        <summary>Verify against a confirmed statement</summary>
        <p>
          <Link to={{ kind: 'formalizations' }}>Browse confirmed statements →</Link>
        </p>
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
