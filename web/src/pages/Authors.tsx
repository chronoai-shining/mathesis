import { useState, type FormEvent } from 'react';
import { Link, useRouter } from '../routing/router';

export function AuthorsPage() {
  const { navigate } = useRouter();
  const [name, setName] = useState('');
  const submit = (event: FormEvent) => {
    event.preventDefault();
    const trimmed = name.trim();
    if (trimmed) navigate({ kind: 'author', name: trimmed });
  };
  return (
    <div className="page narrow-page">
      <header className="page-head">
        <div>
          <p className="eyebrow">Research archive</p>
          <h1>Authors</h1>
          <p className="lede">
            Find a researcher and see their papers, conjectures and formalization work.
          </p>
        </div>
      </header>
      <form className="author-search" role="search" aria-label="Search authors" onSubmit={submit}>
        <label htmlFor="author-search-name">Author name</label>
        <div className="archive-search-input">
          <input
            id="author-search-name"
            type="search"
            value={name}
            maxLength={160}
            placeholder="e.g. Ada Lovelace"
            onChange={(event) => setName(event.target.value)}
          />
          <button className="button" type="submit">
            View author
          </button>
        </div>
      </form>
      <p className="hint">
        Author names are taken from accepted records. You can also find an author by selecting their
        name on any paper or conjecture.
      </p>
      <p>
        <Link to={{ kind: 'papers' }}>Search the full research archive</Link>
      </p>
    </div>
  );
}
