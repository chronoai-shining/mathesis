import contextlib
import io
import json
import os
from pathlib import Path
import tempfile
import types
import unittest
from unittest.mock import patch

import chronocd_setup_nyxid as setup


class CredentialSetupTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.path = Path(self.directory.name) / "private.json"
        self.args = types.SimpleNamespace(profile="owner-test", credentials_file=str(self.path), existing=False)
        self.client = {"id": "test-client", "client_secret": "test-secret-not-real",
                       "client_type": "confidential", "redirect_uris": [setup.CALLBACK],
                       "allowed_scopes": "openid profile email", "is_active": True}
        self.present = {}
        self.calls = []
        self.environment = patch.dict(os.environ, {}, clear=True)
        self.environment.start()
        self.addCleanup(self.environment.stop)

    def invoke(self, arguments, profile, payload=None, output_file=None):
        self.calls.append(arguments)
        if arguments[:2] == ["developer-app", "list"]:
            return {"clients": []}
        if arguments[:2] == ["developer-app", "create"]:
            json.dump(self.client, output_file)
            return None
        if arguments[:2] == ["developer-app", "show"]:
            return self.client
        if arguments[3] == setup.APP_PATH:
            return {"owner": setup.OWNER, "secret_keys_present": self.present}
        service = arguments[3].split("/")[-2]
        self.assertEqual(payload, dict(zip(setup.KEYS, ("test-client", "test-secret-not-real"))))
        self.present[service] = list(setup.KEYS)
        return {"keys": list(setup.KEYS)}

    def test_create_and_provision_without_printing_secret(self):
        output = io.StringIO()
        with patch.object(setup, "invoke", side_effect=self.invoke), contextlib.redirect_stdout(output):
            setup.setup(self.args)
        self.assertEqual(self.path.stat().st_mode & 0o777, 0o600)
        self.assertEqual(set(self.present), set(setup.SERVICES))
        self.assertNotIn(self.client["client_secret"], output.getvalue())
        self.assertIn("--terminal", next(call for call in self.calls if call[:2] == ["developer-app", "create"]))

    def test_resume_does_not_create_another_client(self):
        self.path.write_text(json.dumps(self.client))
        self.path.chmod(0o600)
        with patch.object(setup, "invoke", side_effect=self.invoke), contextlib.redirect_stdout(io.StringIO()):
            setup.setup(self.args)
        self.assertFalse(any(call[:2] == ["developer-app", "create"] for call in self.calls))

    def test_rejects_agent_environment_before_network(self):
        with patch.dict(os.environ, {"NYXID_API_KEY": "test-agent-key"}), patch.object(setup, "invoke") as invoke:
            with self.assertRaisesRegex(RuntimeError, "own computer"):
                setup.setup(self.args)
            invoke.assert_not_called()

    def test_rejects_wrong_owner(self):
        with patch.object(setup, "invoke", return_value={"owner": "different-owner"}):
            with self.assertRaisesRegex(RuntimeError, "Wrong account"):
                setup.setup(self.args)
        self.assertFalse(self.path.exists())

    def test_refuses_partial_configuration_without_recovery_file(self):
        self.present[setup.SERVICES[0]] = list(setup.KEYS)
        with patch.object(setup, "invoke", side_effect=self.invoke):
            with self.assertRaisesRegex(RuntimeError, "partially configured"):
                setup.setup(self.args)

    def test_rejects_publicly_readable_recovery_file(self):
        self.path.write_text(json.dumps(self.client))
        self.path.chmod(0o644)
        with self.assertRaisesRegex(RuntimeError, "0600"):
            setup.load_credentials(self.path)

    def test_rejects_symlink_recovery_file(self):
        destination = Path(self.directory.name) / "target.json"
        destination.write_text(json.dumps(self.client))
        destination.chmod(0o600)
        self.path.symlink_to(destination)
        with self.assertRaises(OSError):
            setup.load_credentials(self.path)

    def test_subprocess_receives_secrets_only_on_stdin(self):
        result = types.SimpleNamespace(returncode=0, stdout='{"keys": []}', stderr="")
        with patch.object(setup.subprocess, "run", return_value=result) as run:
            setup.invoke(["proxy", "request", "chrono-cd", "test", "--data", "-"],
                         "owner-test", payload={"secret": "test-secret-not-real"})
        self.assertNotIn("test-secret-not-real", " ".join(run.call_args.args[0]))
        self.assertIn("test-secret-not-real", run.call_args.kwargs["input"])

    def test_cli_failure_never_includes_secret_response(self):
        result = types.SimpleNamespace(returncode=1, stdout="test-secret-not-real", stderr="test-secret-not-real")
        with patch.object(setup.subprocess, "run", return_value=result):
            with self.assertRaises(RuntimeError) as error:
                setup.invoke(["developer-app", "create"], "owner-test")
        self.assertNotIn("test-secret-not-real", str(error.exception))


if __name__ == "__main__":
    unittest.main()
