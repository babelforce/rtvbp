import pathlib
import re
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[2]
WORKFLOWS = sorted((ROOT / ".github" / "workflows").glob("*.yml"))
ACTION = ROOT / ".github" / "actions" / "setup-ess" / "action.yml"
SETUP_ESS = "uses: ./.github/actions/setup-ess"
JOB = re.compile(r"^  [A-Za-z0-9_-]+:\s*$", re.M)
SHA256 = re.compile(r"\b[0-9a-f]{64}\b")


def jobs(text: str) -> list[str]:
    _, _, body = text.partition("\njobs:\n")
    starts = [m.start() for m in JOB.finditer(body)]
    return [body[a:b] for a, b in zip(starts, starts[1:] + [len(body)])]


def run_blocks(job: str) -> list[tuple[int, str]]:
    """Return (offset, command text) of every `run:` step in a job."""
    blocks = []
    lines = job.splitlines(keepends=True)
    offset = 0
    i = 0
    while i < len(lines):
        m = re.match(r"^(\s*)(?:-\s+)?run:\s*(.*)$", lines[i])
        if m:
            start = offset
            text = m.group(2)
            indent = len(m.group(1))
            j = i + 1
            while j < len(lines) and (not lines[j].strip() or len(lines[j]) - len(lines[j].lstrip()) > indent):
                text += "\n" + lines[j]
                j += 1
            blocks.append((start, text))
        offset += len(lines[i])
        i += 1
    return blocks


def first_ess_user(job: str):
    for offset, text in run_blocks(job):
        if re.search(r"(?:^|[\s;&|(])(?:task check|ess)(?:\s|$)", text, re.M):
            return offset
    return None


class SetupEssTests(unittest.TestCase):
    def test_action_exists_and_pins_one_release(self):
        text = ACTION.read_text()
        self.assertIn("using: composite", text)
        self.assertRegex(text, r"0\.97\.4")
        self.assertEqual(len(SHA256.findall(text)), 1)

    def test_pin_lives_only_in_the_action(self):
        for path in WORKFLOWS:
            text = path.read_text()
            self.assertNotRegex(text, SHA256, f"{path.name} carries an ess hash; use the setup-ess action")
            self.assertNotIn("releases/download", text, f"{path.name} downloads a release itself")

    def test_every_job_that_runs_task_check_or_ess_installs_ess_first(self):
        checked = 0
        for path in WORKFLOWS:
            for job in jobs(path.read_text()):
                use = first_ess_user(job)
                if use is None:
                    continue
                checked += 1
                self.assertIn(SETUP_ESS, job, f"{path.name}: job runs task check or ess without setup-ess")
                self.assertLess(
                    job.index(SETUP_ESS), use, f"{path.name}: setup-ess comes after the first step that needs ess"
                )
        self.assertGreaterEqual(checked, 5)


if __name__ == "__main__":
    unittest.main()
