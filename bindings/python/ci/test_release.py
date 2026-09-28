"""Check that platform documentation tracks the configured ABI3 release jobs."""

from pathlib import Path
import re
import unittest

from platforms import build_matrices, status_targets


class StatusTargetsTest(unittest.TestCase):
    def test_badges_cover_all_and_only_abi3_builds(self):
        builds = [entry for matrix in build_matrices() for entry in matrix]
        expected = {"wheel / " + entry["id"] for entry in builds if "-abi3-" in entry["id"]}
        targets = status_targets()
        self.assertTrue(targets)
        self.assertTrue(all(len(jobs) == 1 for jobs in targets.values()))
        self.assertEqual({name for jobs in targets.values() for name in jobs}, expected)

    def test_both_platform_docs_cover_each_platform_once(self):
        directory = Path(__file__).resolve().parents[1] / "docs"
        for name in ["platforms.md", "platforms_cn.md"]:
            with self.subTest(document=name):
                badges = re.findall(r"/python-build-status/([\w-]+)\.svg", (directory / name).read_text())
                self.assertEqual(len(badges), len(set(badges)))
                self.assertEqual(set(badges), set(status_targets()))


if __name__ == "__main__":
    unittest.main()
