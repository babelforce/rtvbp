import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import textwrap
import unittest


sys.path.insert(0, str(Path(__file__).parent))

import release_tool


class ReleaseToolTests(unittest.TestCase):
    def test_semver_predecessor_stays_inside_component_and_honors_prereleases(self):
        tags = [
            "sdk/go/v0.1.0-rc.3",
            "sdk/rust/v9.0.0",
            "sdk/go/v0.1.0",
            "sdk/go/v0.2.0-rc.1",
            "sdk/go/v0.1.1",
            "protocol/v8.0.0",
        ]

        self.assertEqual(
            release_tool.previous_component_tag(tags, "sdk/go/v0.1.1"),
            "sdk/go/v0.1.0",
        )
        self.assertEqual(
            release_tool.previous_component_tag(tags, "sdk/go/v0.2.0-rc.1"),
            "sdk/go/v0.1.1",
        )
        self.assertIsNone(
            release_tool.previous_component_tag(tags, "sdk/go/v0.1.0-rc.3")
        )

    def test_tag_and_declared_version_must_match(self):
        self.assertEqual(
            str(release_tool.parse_component_tag("rust", "sdk/rust/v1.2.3-rc.1")),
            "1.2.3-rc.1",
        )
        with self.assertRaisesRegex(release_tool.ReleaseError, "expected sdk/rust/v"):
            release_tool.parse_component_tag("rust", "sdk/go/v1.2.3")
        with self.assertRaisesRegex(release_tool.ReleaseError, "invalid semantic version"):
            release_tool.parse_component_tag("protocol", "protocol/v1.2")

    def test_changelog_extracts_only_the_exact_version(self):
        changelog = textwrap.dedent(
            """\
            # Changelog

            ## [1.2.0] - 2026-08-14

            New release.

            ### Added

            - One thing.

            ## [1.1.0] - 2026-08-01

            Previous release.
            """
        )
        self.assertEqual(
            release_tool.extract_changelog_section(changelog, "1.2.0"),
            "New release.\n\n### Added\n\n- One thing.",
        )
        with self.assertRaisesRegex(release_tool.ReleaseError, "version 2.0.0"):
            release_tool.extract_changelog_section(changelog, "2.0.0")

    def test_go_release_has_component_notes_manifest_and_checksums_only(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "source"
            metadata = root / "metadata"
            output = root / "output"
            self._write_common_source(source)
            self._write(source / "sdk/go/go.mod", "module github.com/babelforce/rtvbp/sdk/go\n\ngo 1.24.4\n")
            self._write(
                metadata / "sdk/go/CHANGELOG.md",
                "# Go SDK changelog\n\n## [0.1.1] - 2026-08-14\n\nHardened demo adapter.\n",
            )
            self._commit(source)
            for tag in (
                "sdk/go/v0.1.0-rc.3",
                "sdk/go/v0.1.0",
                "sdk/rust/v9.0.0",
                "sdk/go/v0.1.1",
            ):
                self._git(source, "tag", tag)

            release_tool.build_release("go", "sdk/go/v0.1.1", source, metadata, output)

            assets = sorted(path.name for path in (output / "assets").iterdir())
            self.assertEqual(
                assets,
                [
                    "rtvbp-go-v0.1.1-SHA256SUMS",
                    "rtvbp-go-v0.1.1-release-manifest.json",
                ],
            )
            notes = (output / "release-notes.md").read_text()
            self.assertIn("Hardened demo adapter.", notes)
            self.assertIn("sdk/go/v0.1.0...sdk/go/v0.1.1", notes)
            self.assertNotIn("sdk/go/v0.1.0-rc.3...", notes)
            self.assertNotIn("sdk/rust", notes)

            manifest_path = output / "assets/rtvbp-go-v0.1.1-release-manifest.json"
            manifest = json.loads(manifest_path.read_text())
            self.assertEqual(manifest["component"], "sdk/go")
            self.assertEqual(manifest["commit"], self._git(source, "rev-parse", "HEAD"))
            self.assertEqual(manifest["distribution"]["module"], "github.com/babelforce/rtvbp/sdk/go")
            self.assertEqual(manifest["artifacts"], [])
            self._assert_checksums(output / "assets")

    def test_rust_release_copies_the_crate_and_rejects_version_drift(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "source"
            metadata = root / "metadata"
            output = root / "output"
            self._write_common_source(source)
            self._write(
                source / "sdk/rust/Cargo.toml",
                '[package]\nname = "rtvbp"\nversion = "0.1.0"\n',
            )
            self._write(
                metadata / "sdk/rust/CHANGELOG.md",
                "# Rust SDK changelog\n\n## [0.1.0] - 2026-08-14\n\nFirst Rust SDK.\n",
            )
            crate = root / "rtvbp-0.1.0.crate"
            crate.write_bytes(b"crate bytes")
            self._commit(source)
            self._git(source, "tag", "sdk/go/v0.1.0")
            self._git(source, "tag", "sdk/rust/v0.1.0")

            release_tool.build_release(
                "rust",
                "sdk/rust/v0.1.0",
                source,
                metadata,
                output,
                packaged_artifact=crate,
            )

            copied = output / "assets/rtvbp-0.1.0.crate"
            self.assertEqual(copied.read_bytes(), b"crate bytes")
            self.assertNotIn("compare/", (output / "release-notes.md").read_text())
            manifest = json.loads(
                (output / "assets/rtvbp-rust-v0.1.0-release-manifest.json").read_text()
            )
            self.assertEqual(manifest["artifacts"][0]["sha256"], self._sha256(copied))
            self._assert_checksums(output / "assets")

            self._write(
                source / "sdk/rust/Cargo.toml",
                '[package]\nname = "rtvbp"\nversion = "0.2.0"\n',
            )
            with self.assertRaisesRegex(release_tool.ReleaseError, "Cargo.toml declares 0.2.0"):
                release_tool.build_release(
                    "rust",
                    "sdk/rust/v0.1.0",
                    source,
                    metadata,
                    root / "drift-output",
                    packaged_artifact=crate,
                )

    def test_protocol_bundle_is_deterministic_and_versioned(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "source"
            metadata = root / "metadata"
            self._write_common_source(source)
            self._write(source / "spec/VERSION", "1.0.0\n")
            self._write(
                metadata / "spec/CHANGELOG.md",
                "# Protocol changelog\n\n## [1.0.0] - 2026-08-14\n\nFirst protocol snapshot.\n",
            )
            self._commit(source)
            self._git(source, "tag", "protocol/v1.0.0")

            first = root / "first"
            second = root / "second"
            release_tool.build_release("protocol", "protocol/v1.0.0", source, metadata, first)
            release_tool.build_release("protocol", "protocol/v1.0.0", source, metadata, second)

            archive_name = "rtvbp-protocol-v1.0.0.tar.gz"
            first_archive = first / "assets" / archive_name
            second_archive = second / "assets" / archive_name
            self.assertEqual(first_archive.read_bytes(), second_archive.read_bytes())
            with tarfile.open(first_archive, "r:gz") as archive:
                members = archive.getmembers()
                names = [member.name for member in members]
                self.assertEqual(names, sorted(names))
                self.assertIn(
                    "rtvbp-protocol-v1.0.0/spec/manifests/babelforce.v1.catalog.json",
                    names,
                )
                self.assertIn("rtvbp-protocol-v1.0.0/spec/VERSION", names)
                self.assertIn(
                    "rtvbp-protocol-v1.0.0/conformance/babelforce.v1/scenarios/ping.json",
                    names,
                )
                self.assertTrue(all(member.mtime == 0 for member in members))
                self.assertTrue(all(member.uid == 0 and member.gid == 0 for member in members))

            manifest = json.loads(
                (first / "assets/rtvbp-protocol-v1.0.0-release-manifest.json").read_text()
            )
            self.assertEqual(manifest["distribution"]["kind"], "protocol-bundle")
            self.assertEqual(manifest["artifacts"][0]["sha256"], self._sha256(first_archive))
            self.assertEqual(
                [catalog["id"] for catalog in manifest["catalogs"]],
                ["babelforce.v1", "demo.v1"],
            )
            self._assert_checksums(first / "assets")

    def _write_common_source(self, source):
        self._write(
            source / "spec/manifests/babelforce.v1.catalog.json",
            '{"catalog":{"id":"babelforce.v1"}}\n',
        )
        self._write(
            source / "spec/manifests/demo.v1.catalog.json",
            '{"catalog":{"id":"demo.v1"}}\n',
        )
        self._write(
            source / "conformance/babelforce.v1/scenarios/ping.json",
            '{"scenario":"ping"}\n',
        )
        self._write(
            source / "conformance/demo.v1/scenarios/echo.json",
            '{"scenario":"echo"}\n',
        )
        self._write(source / "conformance/README.md", "# Conformance\n")

    def _commit(self, source):
        source.mkdir(parents=True, exist_ok=True)
        self._git(source, "init", "-q")
        self._git(source, "config", "user.name", "Release Test")
        self._git(source, "config", "user.email", "release@example.com")
        self._git(source, "add", ".")
        subprocess.run(
            ["git", "-C", str(source), "commit", "-q", "-m", "fixture"],
            check=True,
            env={
                **dict(__import__("os").environ),
                "GIT_AUTHOR_DATE": "2026-08-14T08:00:00Z",
                "GIT_COMMITTER_DATE": "2026-08-14T08:00:00Z",
            },
        )

    def _assert_checksums(self, assets):
        checksum_path = next(assets.glob("*-SHA256SUMS"))
        lines = checksum_path.read_text().splitlines()
        self.assertEqual(lines, sorted(lines, key=lambda line: line.split("  ", 1)[1]))
        expected_names = sorted(
            path.name for path in assets.iterdir() if path != checksum_path
        )
        actual_names = []
        for line in lines:
            digest, name = line.split("  ", 1)
            actual_names.append(name)
            self.assertEqual(digest, self._sha256(assets / name))
        self.assertEqual(actual_names, expected_names)

    @staticmethod
    def _write(path, content):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content)

    @staticmethod
    def _git(source, *arguments):
        return subprocess.run(
            ["git", "-C", str(source), *arguments],
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()

    @staticmethod
    def _sha256(path):
        return hashlib.sha256(path.read_bytes()).hexdigest()


if __name__ == "__main__":
    unittest.main()
