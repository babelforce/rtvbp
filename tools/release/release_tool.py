#!/usr/bin/env python3
"""Build deterministic, component-scoped RTVBP GitHub release material."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from functools import total_ordering
import gzip
import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import re
import shutil
import subprocess
import tarfile
import tomllib
from typing import Iterable


REPOSITORY = "https://github.com/babelforce/rtvbp"
COMPONENTS = {
    "go": {
        "tag_prefix": "sdk/go/v",
        "id": "sdk/go",
        "name": "Go SDK",
        "changelog": "sdk/go/CHANGELOG.md",
    },
    "rust": {
        "tag_prefix": "sdk/rust/v",
        "id": "sdk/rust",
        "name": "Rust SDK",
        "changelog": "sdk/rust/CHANGELOG.md",
    },
    "protocol": {
        "tag_prefix": "protocol/v",
        "id": "protocol",
        "name": "Protocol",
        "changelog": "spec/CHANGELOG.md",
    },
}
SEMVER = re.compile(
    r"^(0|[1-9][0-9]*)\."
    r"(0|[1-9][0-9]*)\."
    r"(0|[1-9][0-9]*)"
    r"(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?"
    r"(?:\+([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$"
)


class ReleaseError(ValueError):
    """A release input violates the component release contract."""


@total_ordering
@dataclass(frozen=True)
class SemanticVersion:
    major: int
    minor: int
    patch: int
    prerelease: tuple[str, ...]
    build: tuple[str, ...]

    @classmethod
    def parse(cls, value: str) -> "SemanticVersion":
        match = SEMVER.fullmatch(value)
        if match is None:
            raise ReleaseError(f"invalid semantic version: {value}")
        prerelease = tuple(match.group(4).split(".")) if match.group(4) else ()
        build = tuple(match.group(5).split(".")) if match.group(5) else ()
        for identifier in prerelease:
            if identifier.isdigit() and len(identifier) > 1 and identifier.startswith("0"):
                raise ReleaseError(
                    f"invalid semantic version: numeric prerelease identifier {identifier!r} "
                    "has a leading zero"
                )
        return cls(int(match.group(1)), int(match.group(2)), int(match.group(3)), prerelease, build)

    def __str__(self) -> str:
        value = f"{self.major}.{self.minor}.{self.patch}"
        if self.prerelease:
            value += f"-{'.'.join(self.prerelease)}"
        if self.build:
            value += f"+{'.'.join(self.build)}"
        return value

    def __lt__(self, other: object) -> bool:
        if not isinstance(other, SemanticVersion):
            return NotImplemented
        own_core = (self.major, self.minor, self.patch)
        other_core = (other.major, other.minor, other.patch)
        if own_core != other_core:
            return own_core < other_core
        if not self.prerelease or not other.prerelease:
            return bool(self.prerelease) and not other.prerelease
        for own, candidate in zip(self.prerelease, other.prerelease):
            if own == candidate:
                continue
            own_numeric = own.isdigit()
            candidate_numeric = candidate.isdigit()
            if own_numeric and candidate_numeric:
                return int(own) < int(candidate)
            if own_numeric != candidate_numeric:
                return own_numeric
            return own < candidate
        return len(self.prerelease) < len(other.prerelease)


def parse_component_tag(component: str, tag: str) -> SemanticVersion:
    config = component_config(component)
    prefix = str(config["tag_prefix"])
    if not tag.startswith(prefix):
        raise ReleaseError(f"expected {prefix}<semantic-version>, got {tag}")
    return SemanticVersion.parse(tag.removeprefix(prefix))


def previous_component_tag(tags: Iterable[str], current_tag: str) -> str | None:
    component = component_for_tag(current_tag)
    config = component_config(component)
    prefix = str(config["tag_prefix"])
    current = parse_component_tag(component, current_tag)
    candidates: list[tuple[SemanticVersion, str]] = []
    for tag in tags:
        if tag == current_tag or not tag.startswith(prefix):
            continue
        try:
            version = parse_component_tag(component, tag)
        except ReleaseError:
            continue
        if version < current:
            candidates.append((version, tag))
    return max(candidates, default=(None, None), key=lambda item: item[0])[1]


def extract_changelog_section(changelog: str, version: str) -> str:
    heading = re.compile(
        rf"^## \[{re.escape(version)}\](?:\s+-\s+[^\n]+)?\s*$", re.MULTILINE
    )
    match = heading.search(changelog)
    if match is None:
        raise ReleaseError(f"changelog has no exact section for version {version}")
    next_heading = re.search(r"^##\s+", changelog[match.end() :], re.MULTILINE)
    end = match.end() + next_heading.start() if next_heading else len(changelog)
    section = changelog[match.end() : end].strip()
    if not section:
        raise ReleaseError(f"changelog section for version {version} is empty")
    return section


def build_release(
    component: str,
    tag: str,
    source_root: Path,
    metadata_root: Path,
    output_root: Path,
    *,
    packaged_artifact: Path | None = None,
    repository: str = REPOSITORY,
) -> None:
    source_root = source_root.resolve()
    metadata_root = metadata_root.resolve()
    output_root = output_root.resolve()
    config = component_config(component)
    version = parse_component_tag(component, tag)
    version_text = str(version)
    commit = git(source_root, "rev-parse", "HEAD^{commit}")
    tag_commit = git(source_root, "rev-parse", f"refs/tags/{tag}^{{commit}}")
    if commit != tag_commit:
        raise ReleaseError(
            f"release source HEAD {commit} does not match immutable tag {tag} at {tag_commit}"
        )
    validate_declared_version(component, version_text, source_root)
    if output_root.exists() and any(output_root.iterdir()):
        raise ReleaseError(f"release output directory is not empty: {output_root}")
    assets_root = output_root / "assets"
    assets_root.mkdir(parents=True, exist_ok=True)

    artifact_records: list[dict[str, str]] = []
    if component == "rust":
        if packaged_artifact is None:
            raise ReleaseError("Rust release requires --packaged-artifact")
        packaged_artifact = packaged_artifact.resolve()
        if not packaged_artifact.is_file():
            raise ReleaseError(f"packaged Rust crate does not exist: {packaged_artifact}")
        expected_name = f"rtvbp-{version_text}.crate"
        if packaged_artifact.name != expected_name:
            raise ReleaseError(
                f"packaged Rust crate must be named {expected_name}, got {packaged_artifact.name}"
            )
        target = assets_root / expected_name
        shutil.copyfile(packaged_artifact, target)
        artifact_records.append(artifact_record(target, "application/gzip"))
    elif packaged_artifact is not None:
        raise ReleaseError(f"{component} release does not accept --packaged-artifact")

    if component == "protocol":
        archive = assets_root / f"rtvbp-protocol-v{version_text}.tar.gz"
        build_protocol_archive(source_root, archive, version_text)
        artifact_records.append(artifact_record(archive, "application/gzip"))

    catalogs = catalog_records(source_root)
    manifest_name = f"rtvbp-{component}-v{version_text}-release-manifest.json"
    manifest_path = assets_root / manifest_name
    manifest = {
        "schemaVersion": 1,
        "component": str(config["id"]),
        "version": version_text,
        "tag": tag,
        "commit": commit,
        "sourceDate": git(source_root, "show", "-s", "--format=%cI", commit),
        "repository": repository,
        "distribution": distribution(component, version_text, artifact_records),
        "catalogs": catalogs,
        "artifacts": artifact_records,
    }
    write_json(manifest_path, manifest)

    checksum_path = assets_root / f"rtvbp-{component}-v{version_text}-SHA256SUMS"
    write_checksums(checksum_path, assets_root)

    changelog_path = metadata_root / str(config["changelog"])
    if not changelog_path.is_file():
        raise ReleaseError(f"component changelog does not exist: {changelog_path}")
    changelog_section = extract_changelog_section(changelog_path.read_text(), version_text)
    tags = git(source_root, "tag", "--list").splitlines()
    previous = previous_component_tag(tags, tag)
    notes = render_release_notes(
        component,
        version_text,
        tag,
        previous,
        changelog_section,
        sorted(path.name for path in assets_root.iterdir()),
        repository,
    )
    (output_root / "release-notes.md").write_text(notes)


def build_protocol_archive(source_root: Path, archive_path: Path, version: str) -> None:
    catalog_ids = [record["id"] for record in catalog_records(source_root)]
    files = list((source_root / "spec/manifests").glob("*.catalog.json"))
    files.append(source_root / "spec/VERSION")
    conformance_readme = source_root / "conformance/README.md"
    if conformance_readme.is_file():
        files.append(conformance_readme)
    for catalog_id in catalog_ids:
        catalog_root = source_root / "conformance" / catalog_id
        if not catalog_root.is_dir():
            raise ReleaseError(f"missing conformance directory for catalog {catalog_id}")
        files.extend(path for path in catalog_root.rglob("*") if path.is_file())
    relative_files = sorted({path.relative_to(source_root) for path in files}, key=str)
    prefix = PurePosixPath(f"rtvbp-protocol-v{version}")
    uncompressed = io.BytesIO()
    with tarfile.open(fileobj=uncompressed, mode="w", format=tarfile.PAX_FORMAT) as archive:
        for relative in relative_files:
            source = source_root / relative
            if source.is_symlink():
                raise ReleaseError(f"protocol bundle refuses symbolic link: {relative}")
            data = source.read_bytes()
            info = tarfile.TarInfo(str(prefix / PurePosixPath(relative.as_posix())))
            info.size = len(data)
            info.mode = 0o644
            info.mtime = 0
            info.uid = 0
            info.gid = 0
            info.uname = ""
            info.gname = ""
            archive.addfile(info, io.BytesIO(data))
    uncompressed.seek(0)
    with archive_path.open("wb") as destination:
        with gzip.GzipFile(fileobj=destination, mode="wb", filename="", mtime=0) as compressed:
            shutil.copyfileobj(uncompressed, compressed)


def catalog_records(source_root: Path) -> list[dict[str, str]]:
    records = []
    for path in sorted((source_root / "spec/manifests").glob("*.catalog.json")):
        try:
            catalog_id = json.loads(path.read_text())["catalog"]["id"]
        except (KeyError, TypeError, json.JSONDecodeError) as error:
            raise ReleaseError(f"invalid catalog manifest {path}: {error}") from error
        records.append(
            {
                "id": str(catalog_id),
                "path": path.relative_to(source_root).as_posix(),
                "sha256": sha256_file(path),
            }
        )
    if not records:
        raise ReleaseError("release source contains no spec/manifests/*.catalog.json files")
    return sorted(records, key=lambda record: record["id"])


def distribution(
    component: str, version: str, artifact_records: list[dict[str, str]]
) -> dict[str, str]:
    if component == "go":
        return {
            "kind": "go-module",
            "module": "github.com/babelforce/rtvbp/sdk/go",
            "version": f"v{version}",
        }
    if component == "rust":
        return {
            "kind": "cargo-crate",
            "crate": "rtvbp",
            "version": version,
            "asset": artifact_records[0]["filename"],
        }
    return {
        "kind": "protocol-bundle",
        "version": version,
        "asset": artifact_records[0]["filename"],
    }


def render_release_notes(
    component: str,
    version: str,
    tag: str,
    previous_tag: str | None,
    changelog_section: str,
    assets: list[str],
    repository: str,
) -> str:
    config = component_config(component)
    lines = [f"# RTVBP {config['name']} v{version}", "", changelog_section, ""]
    if component == "go":
        lines.extend(
            [
                "## Install",
                "",
                "```sh",
                f"go get github.com/babelforce/rtvbp/sdk/go@v{version}",
                "```",
                "",
                "The Go module proxy is the canonical SDK distribution; no redundant source archive is attached.",
                "",
            ]
        )
    elif component == "rust":
        lines.extend(
            [
                "## Install",
                "",
                "```sh",
                f"cargo add rtvbp --git {repository} --tag {tag}",
                "```",
                "",
            ]
        )
    else:
        lines.extend(
            [
                "## Bundle",
                "",
                "The protocol archive contains the committed catalog manifests and catalog-owned conformance material.",
                "",
            ]
        )
    lines.extend(["## Verified assets", ""])
    lines.extend(f"- `{asset}`" for asset in assets)
    lines.extend(
        [
            "",
            "Verify checksums with `sha256sum -c <SHA256SUMS-file>` and provenance with "
            f"`gh attestation verify --repo babelforce/rtvbp <asset>`.",
        ]
    )
    if previous_tag is not None:
        lines.extend(
            [
                "",
                f"**Full component changelog:** {repository}/compare/{previous_tag}...{tag}",
            ]
        )
    return "\n".join(lines).rstrip() + "\n"


def validate_declared_version(component: str, version: str, source_root: Path) -> None:
    if component == "go":
        go_mod = source_root / "sdk/go/go.mod"
        module = next(
            (
                line.split(maxsplit=1)[1]
                for line in go_mod.read_text().splitlines()
                if line.startswith("module ")
            ),
            None,
        )
        expected = "github.com/babelforce/rtvbp/sdk/go"
        if module != expected:
            raise ReleaseError(f"sdk/go/go.mod declares module {module!r}, expected {expected!r}")
        return
    if component == "rust":
        cargo_path = source_root / "sdk/rust/Cargo.toml"
        declared = str(tomllib.loads(cargo_path.read_text())["package"]["version"])
        if declared != version:
            raise ReleaseError(
                f"sdk/rust/Cargo.toml declares {declared}, but tag declares {version}"
            )
        return
    declared = (source_root / "spec/VERSION").read_text().strip()
    if declared != version:
        raise ReleaseError(f"spec/VERSION declares {declared}, but tag declares {version}")


def artifact_record(path: Path, media_type: str) -> dict[str, str]:
    return {"filename": path.name, "mediaType": media_type, "sha256": sha256_file(path)}


def write_json(path: Path, value: object) -> None:
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def write_checksums(checksum_path: Path, assets_root: Path) -> None:
    paths = sorted(path for path in assets_root.iterdir() if path != checksum_path)
    checksum_path.write_text(
        "".join(f"{sha256_file(path)}  {path.name}\n" for path in paths)
    )


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def component_for_tag(tag: str) -> str:
    for component, config in COMPONENTS.items():
        if tag.startswith(str(config["tag_prefix"])):
            return component
    expected = ", ".join(str(config["tag_prefix"]) for config in COMPONENTS.values())
    raise ReleaseError(f"tag {tag!r} is outside the component namespaces: {expected}")


def component_config(component: str) -> dict[str, str]:
    try:
        return COMPONENTS[component]
    except KeyError as error:
        raise ReleaseError(f"unknown release component: {component}") from error


def git(root: Path, *arguments: str) -> str:
    try:
        return subprocess.run(
            ["git", "-C", str(root), *arguments],
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()
    except subprocess.CalledProcessError as error:
        message = error.stderr.strip() or error.stdout.strip() or str(error)
        raise ReleaseError(f"git {' '.join(arguments)} failed: {message}") from error


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--component", choices=sorted(COMPONENTS), required=True)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--source-root", type=Path, required=True)
    parser.add_argument("--metadata-root", type=Path, default=Path.cwd())
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--packaged-artifact", type=Path)
    parser.add_argument("--repository", default=REPOSITORY)
    arguments = parser.parse_args()
    try:
        build_release(
            arguments.component,
            arguments.tag,
            arguments.source_root,
            arguments.metadata_root,
            arguments.output,
            packaged_artifact=arguments.packaged_artifact,
            repository=arguments.repository,
        )
    except (OSError, ReleaseError, tomllib.TOMLDecodeError) as error:
        parser.error(str(error))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
