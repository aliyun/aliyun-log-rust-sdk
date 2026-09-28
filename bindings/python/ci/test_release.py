"""Check that platform documentation tracks the configured ABI3 release jobs."""

from pathlib import Path
import re
import unittest

from platforms import build_matrices, ci_matrices, status_targets
from release import check_coverage


class StatusTargetsTest(unittest.TestCase):
    def test_badges_cover_every_build_once(self):
        builds = [entry for matrix in build_matrices() for entry in matrix]
        expected = {"wheel / " + entry["id"] for entry in builds}
        targets = status_targets()
        self.assertTrue(targets)
        self.assertEqual(sum(map(len, targets.values())), len(builds))
        self.assertEqual({name for jobs in targets.values() for name in jobs}, expected)

    def test_both_platform_docs_cover_each_platform_once(self):
        directory = Path(__file__).resolve().parents[1] / "docs"
        for name in ["platforms.md", "platforms_cn.md"]:
            with self.subTest(document=name):
                badges = re.findall(r"/python-build-status/([\w-]+)\.svg", (directory / name).read_text())
                self.assertEqual(len(badges), len(set(badges)))
                self.assertEqual(set(badges), set(status_targets()))


class ReleaseCoverageTest(unittest.TestCase):
    def setUp(self):
        self.entries = [entry for matrix in build_matrices() for entry in matrix]
        self.wheels = [Path("aliyun_log_producer-0.1.0-{}-{}.whl".format(
            entry["tag"], entry["wheel_platform"])) for entry in self.entries]

    def test_runtime_counts_and_unique_artifacts(self):
        from collections import Counter
        self.assertEqual(Counter(entry["kind"] for entry in self.entries),
                         {"abi3": 16, "native": 88, "free-threaded": 15, "pypy": 8, "graalpy": 4})
        self.assertEqual(len(set(self.wheels)), 131)
        check_coverage(self.wheels)

    def test_missing_and_duplicate_cannot_cancel_each_other(self):
        with self.assertRaisesRegex(ValueError, "duplicate"):
            check_coverage(self.wheels[:-1] + [self.wheels[0]])
        with self.assertRaisesRegex(ValueError, "missing"):
            check_coverage(self.wheels[:-1])

    def test_wrong_abi_or_raised_system_baseline_fails(self):
        for old, new in [("cp38-abi3", "cp39-abi3"), ("manylinux2014", "manylinux_2_28")]:
            wheels = list(self.wheels)
            wheels[0] = Path(str(wheels[0]).replace(old, new))
            with self.subTest(replacement=new), self.assertRaisesRegex(ValueError, "unexpected"):
                check_coverage(wheels)

    def test_free_threaded_and_alternate_runtimes_never_test_abi3(self):
        matrices = ci_matrices()
        self.assertTrue(all(row["python"] in ["3.8", "3.9", "3.10", "3.11", "3.12", "3.13", "3.14"]
                            for row in matrices["runtime"]["include"]))
        self.assertEqual({row["kind"] for row in matrices["alternative"]["include"]},
                         {"pypy", "graalpy", "free-threaded"})


if __name__ == "__main__":
    unittest.main()
