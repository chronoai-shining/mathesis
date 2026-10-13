# Mathesis rename and deployment compatibility

Mathesis is the product formerly named Wishpool. The canonical repository is
https://github.com/the-omega-institute/mathesis. Update local remotes with:

```sh
git remote set-url origin https://github.com/the-omega-institute/mathesis.git
```

## Renamed surfaces

Website name, title, metadata, geometric M mark, citations, documentation and
OpenAPI identify Mathesis. The composition crate is `api/crates/mathesis` and
its executable is `mathesis`. Workspace crates are `mathesis-core`,
`mathesis-public`, `mathesis-latex`, `mathesis-review`, `mathesis-verifier`.
The contributor package and executable are `mathesis-contribute`.

Build/install the new executables and update any operator launch commands.
Dockerfiles already launch the new executable. The SDK's `MATHESIS_URL`,
`MATHESIS_TOKEN` and `MATHESIS_NYXID_SERVICE` override the corresponding legacy
`WISHPOOL_*` variables. NyxID service ids are operator registrations, so use the
actual connected id; this rename does not rename an existing NyxID service.

## Persistent compatibility

Keep existing MongoDB databases, collections, GridFS data, OIDC audience and
client registration, identity subjects, session cookies, encryption contexts,
`WP-…` record ids and Lean names (`wishpool_target_prop`, `wishpool_solution`,
`wishpool_disproof`). These are durable interfaces, not display branding.
Single-module digests and idempotency keys are unchanged. New optional fields
have serde defaults and are omitted when empty, preserving old source replays.

Server and verifier environment variables remain `WISHPOOL_*`. This avoids
requiring an unrelated secret/config migration. The local verifier program's
default and infra configuration now name `mathesis-verifier`; update an explicit
operator override if it still points at the old executable.

Existing Kubernetes namespace, resource names, selectors, service DNS, PVC and
published image references remain stable. GitHub repository renaming does not
migrate GHCR packages or publish replacement images. Build and publish a new
image under the chosen existing package before deploying this commit. No
cluster resources or production records are renamed by this source change.

CI changes are confined to matching the renamed crate dependencies, composition
path and local image build tags. Both develop and main continue through the same
checks, runners and branch gates; no checks are disabled or changed in scope.
Production continues to accept changes only through develop → main.

No domain registration, DNS changes, production deployment or NyxID registration
is implied by the GitHub rename. `mathesis.example.org` remains a placeholder.
