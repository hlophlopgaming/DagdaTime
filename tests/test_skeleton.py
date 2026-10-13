"""M0 acceptance checks; run with python3 -m unittest discover -s tests."""
import json
from pathlib import Path
import tomllib
import unittest

ROOT = Path(__file__).resolve().parents[1]


class SkeletonTests(unittest.TestCase):
    def test_workspace_contains_all_four_crates(self):
        manifest = tomllib.loads((ROOT / "Cargo.toml").read_text())
        self.assertEqual(set(manifest["workspace"]["members"]),
                         {"core", "watchers", "daemon", "app"})
        for member in manifest["workspace"]["members"]:
            self.assertTrue((ROOT / member / "Cargo.toml").is_file())

    def test_tauri_has_one_local_window(self):
        config = json.loads((ROOT / "app/tauri.conf.json").read_text())
        self.assertEqual(len(config["app"]["windows"]), 1)
        self.assertEqual(config["app"]["windows"][0]["title"], "DagdaTime")
        frontend = ROOT / "app" / config["build"]["frontendDist"]
        self.assertTrue((frontend / "index.html").is_file())
        self.assertIn("default-src 'self'", config["app"]["security"]["csp"])
        for icon in config["bundle"]["icon"]:
            self.assertTrue((ROOT / "app" / icon).is_file())

    def test_ci_checks_and_builds_on_three_platforms(self):
        workflow = (ROOT / ".github/workflows/ci.yml").read_text()
        for required in ("ubuntu-22.04", "windows-latest", "macos-latest",
                         "cargo fmt --check", "cargo clippy --all-targets -- -D warnings",
                         "cargo test", "cargo build --package dagdatime-app --release"):
            self.assertIn(required, workflow)
