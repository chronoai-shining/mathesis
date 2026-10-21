import argparse
import json
import re
from urllib.parse import urlsplit


def render(image_prefix, image_tag, public_url, admin_subject):
    if not re.fullmatch(r"ghcr\.io/[a-z0-9][a-z0-9-]*/[a-z0-9][a-z0-9._/-]*", image_prefix):
        raise ValueError("image prefix must be a lowercase ghcr.io owner/package")
    if not re.fullmatch(r"sha-[0-9a-f]{40}", image_tag):
        raise ValueError("image tag must pin a full source commit: sha-<40 hex characters>")
    origin = urlsplit(public_url)
    if (
        origin.scheme != "https"
        or not origin.hostname
        or origin.username is not None
        or origin.password is not None
        or origin.path not in ("", "/")
        or origin.query
        or origin.fragment
    ):
        raise ValueError("public URL must be a bare HTTPS origin")
    if not admin_subject.strip() or "," in admin_subject:
        raise ValueError("supply the initial administrator's single NyxID subject")
    common_env = {
        "WISHPOOL_BIND": "0.0.0.0:8080",
        "WISHPOOL_PUBLIC_URL": public_url.rstrip("/"),
        "WISHPOOL_STORAGE": "mongo",
        "WISHPOOL_AUTH_MODE": "nyxid",
        "CHRONO_NYXID_BASE_URL": "https://nyx.chrono-ai.fun",
        "WISHPOOL_ADMIN_SUBJECTS": admin_subject,
        "WISHPOOL_HOSTED_DONATIONS": "false",
        "WISHPOOL_VERIFIER_URL": "http://mathesis-verifier:8081",
    }
    services = {}
    for name, role in (("wishpool-api", "api"), ("mathesis-worker", "worker")):
        services[name] = {
            "image": f"{image_prefix}-api:{image_tag}",
            "port": 8080,
            "health_path": "/readyz",
            "replicas": 1,
            "expose": "internal",
            "ingress_from": ["mathesis-web"] if role == "api" else [],
            "env": {**common_env, "WISHPOOL_ROLE": role},
            "secret_keys": ["CHRONO_NYXID_CLIENT_ID", "CHRONO_NYXID_CLIENT_SECRET"],
            "addon_env": {
                "mongodb": {"uri": "WISHPOOL_MONGODB_URI", "database": "WISHPOOL_MONGODB_DATABASE"}
            },
            "resources": {
                "cpu_request": "100m" if role == "api" else "250m",
                "memory_request": "256Mi" if role == "api" else "512Mi",
                "cpu_limit": "1" if role == "api" else "2",
                "memory_limit": "1Gi" if role == "api" else "2Gi",
            },
            "volumes": [{"name": "tmp", "mount_path": "/tmp", "type": "emptydir", "size": "2Gi"}],
        }
    services["mathesis-worker"]["env"].update({
        "WISHPOOL_TEX_BIN": "/usr/bin",
        "WISHPOOL_TEX_CACHE": "/var/cache/texmf",
        "WISHPOOL_REVIEW_CONCURRENCY": "1",
    })
    services["mathesis-worker"]["volumes"].append({
        "name": "tex-cache", "mount_path": "/var/cache/texmf", "type": "pvc", "size": "1Gi"
    })
    services["mathesis-web"] = {
        "image": f"{image_prefix}-web:{image_tag}",
        "port": 8080,
        "health_path": "/",
        "replicas": 1,
        "routes": ["/"],
        "resources": {
            "cpu_request": "20m", "memory_request": "32Mi",
            "cpu_limit": "250m", "memory_limit": "128Mi",
        },
        "volumes": [{"name": "tmp", "mount_path": "/tmp", "type": "emptydir", "size": "256Mi"}],
    }
    services["mathesis-verifier"] = {
        "image": f"{image_prefix}-verifier:{image_tag}",
        "port": 8081,
        "health_path": "/readyz",
        "replicas": 1,
        "expose": "internal",
        "egress": "none",
        "ingress_from": ["wishpool-api", "mathesis-worker"],
        "env": {
            "HOME": "/opt/verifier", "TMPDIR": "/scratch",
            "WISHPOOL_LEAN_WORKSPACE": "/opt/lean-workspace",
            "WISHPOOL_VERIFIER_BIND": "0.0.0.0:8081",
            "WISHPOOL_VERIFIER_TIMEOUT_SECS": "1200",
        },
        "resources": {
            "cpu_request": "500m", "memory_request": "1Gi",
            "cpu_limit": "2", "memory_limit": "8Gi",
        },
        "volumes": [{"name": "scratch", "mount_path": "/scratch", "type": "emptydir", "size": "24Gi"}],
    }
    return {"addons": {"mongodb": {}}, "services": services}


def main():
    parser = argparse.ArgumentParser(description="Render the secret-free Mathesis ChronoCD spec")
    parser.add_argument("--image-prefix", required=True)
    parser.add_argument("--image-tag", required=True)
    parser.add_argument("--public-url", required=True)
    parser.add_argument("--admin-subject", required=True)
    args = parser.parse_args()
    try:
        spec = render(args.image_prefix, args.image_tag, args.public_url, args.admin_subject)
    except ValueError as error:
        parser.error(str(error))
    print(json.dumps(spec, indent=2))


if __name__ == "__main__":
    main()
