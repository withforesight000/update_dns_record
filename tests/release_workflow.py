"""Run the actual release-resume shell step with a local GitHub CLI fake."""

import json
import os
from pathlib import Path
import subprocess
import tempfile
import textwrap
import unittest


ROOT = Path(__file__).resolve().parents[1]
WORKFLOW = (ROOT / ".github/workflows/release.yml").read_text()
STEP = textwrap.dedent(
    WORKFLOW.split("      - name: Create or resume draft and upload assets\n", 1)[1]
    .split("        run: |\n", 1)[1]
    .split("      - name:", 1)[0]
)
FAKE_GH = """#!/usr/bin/env python3
import json
import os
from pathlib import Path
import sys
args = sys.argv[1:]
with open(os.environ['CALLS'], 'a') as log:
    log.write(json.dumps(args) + '\\n')
if args[0] == 'api':
    if os.environ['FAIL_API'] == '1':
        sys.exit(1)
    pages = json.loads(Path(os.environ['PAGES']).read_text())
    for page in pages if '--paginate' in args else pages[:1]:
        print(json.dumps(page))
elif args[:2] not in [['release', 'create'], ['release', 'upload']]:
    sys.exit('Unexpected gh invocation')
"""


class ReleaseResumeTests(unittest.TestCase):
    def run_step(self, pages, fail_api=False):
        with tempfile.TemporaryDirectory() as directory:
            work = Path(directory)
            gh = work / "gh"
            gh.write_text(FAKE_GH)
            gh.chmod(0o755)
            (work / "pages.json").write_text(json.dumps(pages))
            (work / "dist").mkdir()
            for asset in ["amd64.tar.gz", "arm64.tar.gz", "SHA256SUMS"]:
                (work / "dist" / asset).touch()
            env = os.environ.copy()
            env.update(
                PATH=f"{work}{os.pathsep}{env['PATH']}",
                GH_REPO="example/repo",
                VERSION="1.1.0",
                TAG_COMMIT="expected-commit",
                CALLS=str(work / "calls.jsonl"),
                PAGES=str(work / "pages.json"),
                FAIL_API="1" if fail_api else "0",
            )
            result = subprocess.run(
                ["bash", "-e", "-o", "pipefail", "-c", STEP],
                cwd=work, env=env, capture_output=True, text=True,
            )
            calls = [json.loads(line) for line in (work / "calls.jsonl").read_text().splitlines()]
            return result, calls

    @staticmethod
    def release(draft=True, commit="expected-commit"):
        return {"id": 42, "tag_name": "1.1.0", "draft": draft, "target_commitish": commit}

    def test_resumes_draft_on_later_page_without_creating_release(self):
        first_page = [{"id": i, "tag_name": f"2.0.{i}"} for i in range(30)]
        result, calls = self.run_step([first_page, [self.release()]])
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(len(calls), 2)
        self.assertEqual(calls[1][:3], ["release", "upload", "1.1.0"])

    def test_creates_draft_only_when_tag_is_absent_from_all_pages(self):
        result, calls = self.run_step([[{"tag_name": "2.0.0"}], []])
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(calls[1][:3], ["release", "create", "1.1.0"])
        self.assertIn("--draft", calls[1])
        self.assertEqual(calls[2][:3], ["release", "upload", "1.1.0"])

    def test_published_release_on_later_page_is_not_overwritten(self):
        result, calls = self.run_step([[], [self.release(draft=False)]])
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(len(calls), 1)

    def test_draft_for_different_commit_is_not_overwritten(self):
        result, calls = self.run_step([[], [self.release(commit="other-commit")]])
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(len(calls), 1)

    def test_listing_failure_does_not_create_or_upload(self):
        result, calls = self.run_step([[]], fail_api=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(len(calls), 1)


if __name__ == "__main__":
    unittest.main()
