# Mathesis on ChronoCD

This integration was prepared by **CMA operating on behalf of the repository
user**. Validate changes on the fork's `cma/chrono-cd` branch with a pull request
into the fork's `develop`. Do not merge into the upstream repository until the
deployment is verified and the owner authorizes upstream integration. This
runbook does not merge or deploy to `main` automatically.

## Images

The `ChronoCD images` workflow publishes the existing API/worker, web, prepared
Lean and tested verifier images to `ghcr.io/<repository-owner>/mathesis-*` on
pushes to `develop`, `main` or `cma/chrono-cd`, or manual dispatch. Tags are `sha-<full commit>`;
there is no moving `latest` deployment tag. The verifier build uses the prepared
Lean image's digest and runs the existing real-Lean tests before publication.
All runtime images use non-root users. No application credentials enter builds.

Wait for all image jobs to succeed. A fork publishes under its own owner, not the
upstream organization. Newly created GHCR packages may be private: configure the
owner's GitHub connection for private package pulls through the approved NyxID
credential flow. Do not paste tokens into chat or commit registry credentials.
Package publication does not itself deploy anything.

The API CI job uses Ubuntu 22.04 while MongoDB 8.0 rejects Linux kernels 6.19
and newer (MongoDB SERVER-121912). Its MongoDB service must pass a ping health
check before the job starts. Keep this compatibility constraint until a tested
MongoDB release supports the newer runner kernel; do not skip database tests.

## Account and configuration

Use the installed `chrono-cd` Ornn skill and the authorized NyxID CLI:

```bash
nyxid proxy request chrono-cd api/v1/account --output json
nyxid proxy request chrono-cd api/v1/projects --output json
```

Choose the project/app name and check that `wishpool-api`, `mathesis-web`,
`mathesis-worker` and `mathesis-verifier` are unused within the selected owner.
The API name intentionally matches the existing web image's Nginx upstream.
Personal scope is the default; for organization scope, verify admin membership
and pass `?org_id=<id>` on every request. Do not overwrite an existing app.

Derive the origin from the account's slug and chosen app name, then confirm it
against the app response. The standard hostname is
`https://<app>-<owner-slug>.apps.chrono-ai.fun`.

```bash
python3 infra/chronocd/render.py \
  --image-prefix "ghcr.io/$IMAGE_OWNER/mathesis" \
  --image-tag "sha-$SOURCE_COMMIT" \
  --public-url "$PUBLIC_URL" \
  --admin-subject "$ADMIN_SUBJECT" > /tmp/mathesis-chronocd.json
nyxid proxy request chrono-cd "api/v1/projects/$PROJECT/apps/$APP" \
  --method PUT --data "$(cat /tmp/mathesis-chronocd.json)" --output json
```

The spec contains no secret values. ChronoCD injects a separate managed MongoDB
database into the API and worker; do not replace that database on later updates.
The two roles use production NyxID authentication. Only the web service is public;
Nginx proxies `/api/` and `/auth/` with the existing upload limits and timeouts.
The one-replica worker has a persistent TeX cache. The credential-free Lean
verifier accepts traffic only from the API/worker and has no outbound network.
Requests including one surge per service total 1.74 CPU and 3648 MiB; storage is
1 GiB of persistent storage and at most eight service pods. The verifier also
needs 24 GiB of ephemeral scratch space for its full per-request workspace copy
and retains the existing verifier manifest's 8 GiB memory limit. Check node
capacity and other apps' usage before applying.

## Sign-in credentials: owner action required

Create a **confidential** NyxID developer application with scopes
`openid profile email` and redirect URI `$PUBLIC_URL/auth/callback`. NyxID's
developer-client API requires a human user session and rejects agent API keys;
additional proxy-service authorization does not bypass this restriction.

For this fork deployment, an owner-only helper creates the confidential client
and provisions both services without printing credentials. On your own computer
(not in the CMA agent's Sandbox), with Python 3 and the NyxID CLI installed, run:

```bash
nyxid login --profile mathesis-owner --base-url https://nyx-api.chrono-ai.fun
python3 docs/chronocd_setup_nyxid.py
```

Sign in as the owner of `mathesis/mathesis`. The helper checks that identity,
uses the exact deployed callback, and sends secrets over stdin rather than command
arguments. It retains a mode-0600 recovery file at `~/.mathesis-oidc-private.json`
outside Git; rerun the same command to resume a partial setup without creating
another client. Store the recovery file securely or remove it after successful
verification. Never upload it or paste its contents into chat. If you already
created the OAuth client, use `--existing` to enter its ID and saved secret into
hidden prompts on your own terminal. No existing client is rotated or deleted.

The helper configures credentials only: it does not approve quota, verify login,
enable automatic reviews, or prove the application is ready. Run its offline
safety tests with `python3 -B -m unittest discover -s docs -p 'test_chronocd_setup_nyxid.py'`.

Provision `CHRONO_NYXID_CLIENT_ID` and `CHRONO_NYXID_CLIENT_SECRET` into both
`wishpool-api` and `mathesis-worker` through ChronoCD's service secrets endpoint
or an owner-controlled secret provisioning flow:

`PUT api/v1/projects/<project>/apps/<app>/services/<service>/secrets`

Never send the values through chat, print them, or place them in the spec or Git.
Only the owner should handle the secret input, using their approved credential
flow. `422 missing_secret_keys` means the app spec was saved but these services
have not started; it is not a successful deployment.

## Automatic reviews and CMA

The baseline starts the real web/API, durable storage, TeX worker and isolated
solution verifier. It does **not** claim the automated research pipeline is
enabled: Oracle, managed CMA execution and delegated donations require separate
operator provisioning described in the main README.

Before admitting automated reviews, configure the full API/worker spec with
`WISHPOOL_ORACLE=http`, its separately provisioned secret, a ready
`WISHPOOL_CMA_WORKSPACE`, exact published `WISHPOOL_CMA_AGENT_PROFILE`, HTTP CMA
transport and `WISHPOOL_CMA_TOKEN_FILE`. Mount the token through ChronoCD secret
files outside scratch/PVC paths. CMA requires an ordinary unrestricted **user**
token, not this CMA agent's key. Never copy the operating agent's credential into
the application. Lean target generation also requires the prepared pinned
workspace in the API/worker runtime; the standalone verifier image alone does
not supply that workspace to those containers. Keep automation disabled until
those prerequisites and the application's `--check` preflight succeed. Resource
readiness alone does not prove provider quota or end-to-end review execution.

## Verify, update and roll back

```bash
nyxid proxy request chrono-cd "api/v1/projects/$PROJECT/apps/$APP/status" --output json
nyxid proxy request chrono-cd "api/v1/projects/$PROJECT/apps/$APP/logs?service=wishpool-api&tail=100" --output json
```

Poll status every 15 seconds for up to eight minutes. Require `phase: ready`,
ready pods for every service and `route.serving: true`, then check the public web
page, an API JSON response and the sign-in redirect. Test a small consented source
upload as an authenticated owner to verify PDF compilation; do not claim this
from HTTP health alone. Do not expose logs that might contain private papers.

Before updates, GET the current app and preserve its full spec, add-ons, secrets,
organization scope and operator settings. PUT replaces the entire spec; do not
reapply a fresh baseline over a customized app. Use `If-Match` with the observed
generation to avoid concurrent overwrites. Update all image references to a
tested source commit. ChronoCD resolves and records immutable image digests.
Rollback by restoring the prior revision's spec/image tags, not by deleting the
app or database. Never use `purge=true` as part of a rollout.
