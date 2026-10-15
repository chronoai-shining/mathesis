# Project showcase on GitHub Pages

The showcase is a separate static site in `web/showcase`. It explains Mathesis,
links to the source and documentation, and optionally links to a running service.
It does not load the application, call its API, accept submissions or contain
credentials. GitHub Pages cannot run Rust, MongoDB, review workers or Lean.

## Local preview

Use Node 22. From `web/`:

```sh
npm ci
npm run build:showcase
npm run preview:showcase
```

Open `http://127.0.0.1:4173/mathesis/`. The default asset base is `/mathesis/`.
For live editing, use `npm run dev:showcase` and port 5173 instead.
For a domain root, build with `PAGES_BASE_PATH=/ npm run build:showcase`.
The main application remains `npm run dev` / `npm run build`.

## Publish

After reviewing and merging the changes:

1. In repository **Settings → Pages**, select **GitHub Actions** as the source.
2. Allow `develop` (or `main`) in the `github-pages` environment's deployment
   branch policy. Keep any desired environment reviewers.
3. In **Actions → Project showcase**, run the workflow on that branch.
4. Use the deployment URL reported by the workflow. With no custom domain, this
   repository's default URL is `https://the-omega-institute.github.io/mathesis/`.

Pull requests build an artifact for review. Only an explicit workflow dispatch
from `develop` or `main` deploys it. The URL above is the expected address; it is
not a claim that Pages has already been enabled or published.

## Add a domain later

No domain is required now, and the default build contains no `CNAME` file.
Once a domain is chosen:

1. Verify domain ownership in the GitHub organization/user Pages settings.
2. For a subdomain such as `mathesis.example.com`, create a DNS CNAME pointing
   to `the-omega-institute.github.io` (no scheme and no `/mathesis/` suffix).
   Apex domains require GitHub's documented A/AAAA or ALIAS/ANAME configuration.
3. Set the repository Actions variable `PAGES_CUSTOM_DOMAIN` to the full hostname.
4. Configure the same custom domain in **Settings → Pages** and enable HTTPS
   after GitHub issues its certificate.
5. Dispatch the workflow again. It builds for `/` and writes the matching CNAME.

Keep the Pages setting and variable in sync when adding or removing a domain.
See [GitHub's custom domain instructions](https://docs.github.com/en/pages/configuring-a-custom-domain-for-your-github-pages-site).

When a backend deployment is available, set the Actions variable
`MATHESIS_APP_URL` to its HTTPS public URL. The showcase then displays an
“Open the platform” link. Locally the equivalent build variable is
`VITE_MATHESIS_APP_URL`. These are public URLs, never secrets. The live application
should serve its API/auth on its own origin as described in the deployment docs.
