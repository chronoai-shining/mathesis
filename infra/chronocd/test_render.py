import unittest

from render import render


class RenderTests(unittest.TestCase):
    def make_spec(self, **overrides):
        options = {
            "image_prefix": "ghcr.io/example/mathesis",
            "image_tag": "sha-" + "a" * 40,
            "public_url": "https://mathesis-example.apps.chrono-ai.fun",
            "admin_subject": "test-admin-subject",
        }
        return render(**(options | overrides))

    def test_production_roles_share_managed_database_and_auth(self):
        spec = self.make_spec()
        self.assertEqual(spec["addons"], {"mongodb": {}})
        for name, role in (("wishpool-api", "api"), ("mathesis-worker", "worker")):
            service = spec["services"][name]
            self.assertEqual(service["env"]["WISHPOOL_ROLE"], role)
            self.assertEqual(service["env"]["WISHPOOL_AUTH_MODE"], "nyxid")
            self.assertEqual(service["env"]["WISHPOOL_STORAGE"], "mongo")
            self.assertNotIn("WISHPOOL_MONGODB_URI", service["env"])
            self.assertEqual(service["addon_env"]["mongodb"]["uri"], "WISHPOOL_MONGODB_URI")
            self.assertEqual(service["secret_keys"], ["CHRONO_NYXID_CLIENT_ID", "CHRONO_NYXID_CLIENT_SECRET"])
            self.assertFalse(set(service["secret_keys"]) & service["env"].keys())
            self.assertEqual(service["health_path"], "/readyz")

    def test_web_is_only_public_service_and_retains_nginx_upstream(self):
        services = self.make_spec()["services"]
        self.assertEqual([name for name, service in services.items() if service.get("routes")], ["mathesis-web"])
        self.assertEqual(services["mathesis-web"]["routes"], ["/"])
        self.assertEqual(services["wishpool-api"]["port"], 8080)
        self.assertEqual(services["wishpool-api"]["ingress_from"], ["mathesis-web"])

    def test_verifier_is_credential_free_and_isolated(self):
        verifier = self.make_spec()["services"]["mathesis-verifier"]
        self.assertEqual(verifier["egress"], "none")
        self.assertEqual(verifier["expose"], "internal")
        self.assertEqual(verifier["ingress_from"], ["wishpool-api", "mathesis-worker"])
        for field in ("secret_keys", "addon_env", "routes"):
            self.assertNotIn(field, verifier)

    def test_worker_has_one_replica_and_persistent_tex_cache(self):
        worker = self.make_spec()["services"]["mathesis-worker"]
        self.assertEqual(worker["replicas"], 1)
        caches = [volume for volume in worker["volumes"] if volume["type"] == "pvc"]
        self.assertEqual(len(caches), 1)
        self.assertEqual(caches[0]["mount_path"], worker["env"]["WISHPOOL_TEX_CACHE"])

    def test_requests_fit_default_quota_including_surge(self):
        services = self.make_spec()["services"].values()
        cpu = sum(int(service["resources"]["cpu_request"].removesuffix("m")) * 2 for service in services)
        memory = sum(
            (int(service["resources"]["memory_request"][:-2]) *
             (1024 if service["resources"]["memory_request"].endswith("Gi") else 1)) * 2
            for service in services
        )
        self.assertLessEqual(cpu, 2000)
        self.assertLessEqual(memory, 4096)
        self.assertLessEqual(len(list(services)) * 2, 10)

    def test_images_use_the_same_source_commit(self):
        for service in self.make_spec()["services"].values():
            self.assertTrue(service["image"].endswith(":sha-" + "a" * 40))

    def test_rejects_unpinned_images_and_unsafe_origins(self):
        cases = [
            {"image_prefix": "docker.io/example/mathesis"},
            {"image_tag": "latest"},
            {"image_tag": "sha-abc123"},
            {"public_url": "http://example.com"},
            {"public_url": "https://example.com/auth"},
            {"public_url": "https://user:password@example.com"},
            {"public_url": "https://example.com?query=yes"},
            {"public_url": "https://example.com#fragment"},
            {"admin_subject": ""},
        ]
        for options in cases:
            with self.subTest(options=options), self.assertRaises(ValueError):
                self.make_spec(**options)


if __name__ == "__main__":
    unittest.main()
