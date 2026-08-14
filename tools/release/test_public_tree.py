import hashlib
import json
import pathlib
import re
import subprocess
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[2]

# Opaque denylist entries keep confidential host/repository identifiers out of this public file.
# The tuple is (identifier length, minimum separator count, separator, sha256(lowercase identifier)).
CONFIDENTIAL_IDENTIFIERS = (
    (27, 2, ".", "e136da79c414f4dabec30e690b915af2c60fb4013f78d02c7d86343a1ae50454"),
    (17, 2, "-", "f21c3a4428cd4747bd426190a9a56600c82e699012047377271ce6539076edb8"),
)
TOKEN = re.compile(r"[A-Za-z0-9._-]+")
SCP_STYLE_GIT = re.compile(r"\bgit" + r"@[A-Za-z0-9.-]+:")


def public_files() -> list[pathlib.Path]:
    result = subprocess.run(
        ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"],
        cwd=ROOT,
        check=True,
        capture_output=True,
    )
    return [ROOT / path.decode() for path in result.stdout.split(b"\0") if path]


def confidential_identifier_present(text: str) -> bool:
    for token_match in TOKEN.finditer(text.lower()):
        candidate = token_match.group()
        for length, minimum_separators, separator, expected_hash in CONFIDENTIAL_IDENTIFIERS:
            if len(candidate) < length or candidate.count(separator) < minimum_separators:
                continue
            for start in range(len(candidate) - length + 1):
                window = candidate[start : start + length]
                if window.count(separator) < minimum_separators:
                    continue
                if hashlib.sha256(window.encode()).hexdigest() == expected_hash:
                    return True
    return False


def private_locator_present(text: str) -> bool:
    lowered = text.lower()
    return (
        "git+" + "s" + "sh" + "://" in lowered
        or "ssh" + "://" in lowered
        or SCP_STYLE_GIT.search(text) is not None
    )


class PublicTreeTest(unittest.TestCase):
    def test_npm_lockfiles_use_only_the_public_default_registry(self) -> None:
        violations: list[str] = []
        for path in public_files():
            if path.name not in {"package-lock.json", "npm-shrinkwrap.json"}:
                continue
            lockfile = json.loads(path.read_text(encoding="utf-8"))
            for package, metadata in lockfile.get("packages", {}).items():
                if not isinstance(metadata, dict) or "resolved" not in metadata:
                    continue
                resolved = metadata["resolved"]
                if not isinstance(resolved, str) or not resolved.startswith(
                    "https://registry.npmjs.org/"
                ):
                    violations.append(f"{path.relative_to(ROOT)}:{package}")

        self.assertEqual(
            violations,
            [],
            "npm lockfile entries must resolve through the public default registry",
        )

    def test_public_tree_contains_no_private_source_coordinates(self) -> None:
        violations: list[str] = []
        for path in public_files():
            if not path.is_file():
                continue
            data = path.read_bytes()
            if b"\0" in data:
                continue
            text = data.decode("utf-8", errors="replace")
            if confidential_identifier_present(text) or private_locator_present(text):
                violations.append(str(path.relative_to(ROOT)))

        self.assertEqual(violations, [], "private source coordinates found in public files")

    def test_coordinate_detectors_are_live(self) -> None:
        self.assertTrue(
            private_locator_present(
                "git+" + "s" + "sh" + "://" + "git" + "@example.invalid/x.git"
            )
        )
        self.assertTrue(private_locator_present("git" + "@example.invalid:x.git"))
        self.assertFalse(private_locator_present("https://github.com/babelforce/rtvbp.git"))


if __name__ == "__main__":
    unittest.main()
