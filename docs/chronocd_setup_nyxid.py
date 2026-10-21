import argparse
import getpass
import json
import os
from pathlib import Path
import stat
import subprocess
import sys


BASE_URL = "https://nyx-api.chrono-ai.fun"
OWNER = "76fe9d91-1f1d-4234-9352-819a7c28f709"
APP_PATH = "api/v1/projects/mathesis/apps/mathesis"
CALLBACK = "https://mathesis-c1ddd1707b.apps.chrono-ai.fun/auth/callback"
SERVICES = ("wishpool-api", "mathesis-worker")
KEYS = ("CHRONO_NYXID_CLIENT_ID", "CHRONO_NYXID_CLIENT_SECRET")


def invoke(arguments, profile, payload=None, output_file=None):
    command = ["nyxid", *arguments, "--profile", profile,
               "--base-url", BASE_URL, "--output", "json"]
    result = subprocess.run(
        command, input=json.dumps(payload) if payload is not None else None,
        text=True, stdout=output_file or subprocess.PIPE, stderr=subprocess.PIPE,
        timeout=120, check=False,
    )
    if result.returncode:
        raise RuntimeError("NyxID request failed; response withheld to protect credentials.")
    return None if output_file else json.loads(result.stdout)


def validate_client(client):
    scopes = client.get("allowed_scopes", "")
    scopes = scopes.split() if isinstance(scopes, str) else scopes
    if (client.get("client_type") != "confidential"
            or CALLBACK not in client.get("redirect_uris", [])
            or not {"openid", "profile", "email"}.issubset(scopes)
            or client.get("is_active") is False):
        raise RuntimeError("Client must be active/confidential with the Mathesis callback and OIDC scopes.")


def load_credentials(path):
    descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    with os.fdopen(descriptor) as source:
        metadata = os.fstat(source.fileno())
        if (not stat.S_ISREG(metadata.st_mode) or metadata.st_uid != os.getuid()
                or stat.S_IMODE(metadata.st_mode) & 0o077):
            raise RuntimeError("Credential file must be owned by you with permissions 0600.")
        return json.load(source)


def setup(args):
    if os.environ.get("NYXID_ACCESS_TOKEN") or os.environ.get("NYXID_API_KEY"):
        raise RuntimeError("Run on your own computer using a browser-login profile, not an agent/token environment.")
    app = invoke(["proxy", "request", "chrono-cd", APP_PATH], args.profile)
    if app.get("owner") != OWNER:
        raise RuntimeError("Wrong account: sign in as the owner of this Mathesis deployment.")
    present = app.get("secret_keys_present", {})
    if all(set(KEYS).issubset(present.get(service, [])) for service in SERVICES):
        print("Both services already have the credential keys. Nothing changed; login is not yet verified.")
        return
    path = Path(args.credentials_file).expanduser()
    if any((parent / ".git").exists() for parent in path.resolve().parents):
        raise RuntimeError("Keep the private credential file outside all Git workspaces.")
    if not path.exists() and any(set(KEYS).intersection(present.get(service, [])) for service in SERVICES):
        raise RuntimeError("Credentials are partially configured. Resume with the original private credential file; do not replace them.")
    if not path.exists():
        clients = invoke(["developer-app", "list"], args.profile)
        matches = [client for client in clients.get("clients", [])
                   if CALLBACK in client.get("redirect_uris", [])]
        if matches and not args.existing:
            raise RuntimeError("A client already uses this callback. Run with --existing and its saved secret; no duplicate created.")
        descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
        with os.fdopen(descriptor, "w") as destination:
            if args.existing:
                client_id = getpass.getpass("Existing client ID (hidden): ")
                client = invoke(["developer-app", "show", client_id], args.profile)
                validate_client(client)
                client["client_secret"] = getpass.getpass("Existing client secret (hidden): ")
                json.dump(client, destination)
            else:
                invoke([
                    "developer-app", "create", "--name", "Mathesis fork validation",
                    "--client-type", "confidential", "--redirect-uri", CALLBACK,
                    "--allowed-scopes", "openid profile email", "--delegation-scopes", "",
                    "--broker-capability", "false", "--terminal",
                ], args.profile, output_file=destination)
            destination.flush()
            os.fsync(destination.fileno())
    credentials = load_credentials(path)
    client_id = credentials.get("id")
    secret = credentials.get("client_secret")
    if not isinstance(client_id, str) or not client_id or not isinstance(secret, str) or not secret:
        raise RuntimeError("Credential file lacks the client ID/secret. Inspect Developer apps before retrying creation.")
    validate_client(invoke(["developer-app", "show", client_id], args.profile))
    payload = dict(zip(KEYS, (client_id, secret)))
    failures = []
    for service in SERVICES:
        try:
            invoke(["proxy", "request", "chrono-cd", f"{APP_PATH}/services/{service}/secrets",
                    "--method", "PUT", "--data", "-"], args.profile, payload=payload)
        except RuntimeError:
            failures.append(service)
    app = invoke(["proxy", "request", "chrono-cd", APP_PATH], args.profile)
    present = app.get("secret_keys_present", {})
    if not all(set(KEYS).issubset(present.get(service, [])) for service in SERVICES):
        raise RuntimeError("Not all credential keys are confirmed. Keep the private file and rerun to resume.")
    print("Credential keys are present in both services. No credential values were printed.")
    if failures:
        print("A secrets request reported an error after storage; deployment reconciliation still needs checking.")
    print(f"Private recovery file retained at {path}. Keep it out of Git/chat; store it securely or remove it after verification.")
    print("This does not verify login or deployment health; quota approval and rollout checks remain separate.")


def main():
    parser = argparse.ArgumentParser(description="Owner-only Mathesis OIDC setup; run on your own computer.")
    parser.add_argument("--profile", default="mathesis-owner")
    parser.add_argument("--credentials-file", default=str(Path.home() / ".mathesis-oidc-private.json"))
    parser.add_argument("--existing", action="store_true", help="Use an existing client via hidden terminal prompts.")
    args = parser.parse_args()
    try:
        setup(args)
    except (RuntimeError, OSError, ValueError, subprocess.TimeoutExpired) as error:
        message = str(error) if isinstance(error, RuntimeError) else type(error).__name__
        print(f"Setup stopped: {message}", file=sys.stderr)
        print("Any private recovery file is retained. Do not paste it or raw CLI responses into chat.", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
