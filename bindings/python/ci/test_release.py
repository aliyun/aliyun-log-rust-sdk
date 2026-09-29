"""Check that platform documentation tracks the configured ABI3 release jobs."""

from collections import Counter
import io
from pathlib import Path
import re
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import zipfile

from platforms import build_groups, build_matrices, ci_matrices, status_targets
from release import check_coverage, check_dist, check_group, desktop_interpreters, prepare


class StatusTargetsTest(unittest.TestCase):
    def test_badges_cover_every_build_group_once(self):
        groups = [group for matrix in build_groups() for group in matrix]
        expected = {"wheel / " + group["id"] for group in groups}
        targets = status_targets()
        self.assertTrue(targets)
        self.assertEqual(sum(map(len, targets.values())), len(groups))
        self.assertEqual({name for jobs in targets.values() for name in jobs}, expected)
        for group in groups:
            badge = group["platform"] + ("" if group["kind"] == "abi3" else "-" + group["kind"])
            self.assertIn("wheel / " + group["id"], targets[badge])

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
                         {"abi3": 16, "native": 88, "free-threaded": 15, "pypy": 8, "graalpy": 7})
        self.assertEqual(len(set(self.wheels)), 134)
        check_coverage(self.wheels)

    def test_missing_and_duplicate_cannot_cancel_each_other(self):
        with self.assertRaisesRegex(ValueError, "duplicate"):
            check_coverage(self.wheels[:-1] + [self.wheels[0]])
        with self.assertRaisesRegex(ValueError, "missing"):
            check_coverage(self.wheels[:-1])

    def test_graalpy_versions_and_legacy_intel_exception(self):
        graalpy = [row for row in self.entries if row["kind"] == "graalpy"]
        for row in graalpy:
            intel = row["platform"] == "x86_64-apple-darwin"
            with self.subTest(job=row["id"]):
                if intel:
                    self.assertEqual(row["python"], "3.12")
                    self.assertEqual(row["setup_python"], "graalpy-25.0.1")
                elif row["python"] == "3.12":
                    self.assertEqual(row["setup_python"], "graalpy-25.2.4")
                else:
                    self.assertEqual(row["setup_python"], "graalpy-25.4.4")
                if row["python"] == "3.13":
                    self.assertEqual(row["tag"], "graalpy313-graalpy253_313_native")
                if "policy" in row:
                    self.assertEqual(row["interpreter"], "graalpy" + row["python"])
                    self.assertEqual(row["policy"], "manylinux2014" if row["python"] == "3.12" else "manylinux_2_28")
                    self.assertNotIn("graalpy_url", row)

        # Each test job must agree with the release runtime on that same host.
        desktop = {(row["runner"], row["python"]): row for row in graalpy if "runner" in row}
        alternative = ci_matrices()["alternative"]["include"]
        identities = [(row["os"], row["kind"], row["python"]) for row in alternative]
        self.assertEqual(len(identities), len(set(identities)))
        for row in alternative:
            if row["kind"] == "graalpy" and row["os"].startswith("macos"):
                self.assertEqual(row["setup_python"], desktop[row["os"], row["python"]]["setup_python"])

    def test_wrong_abi_or_raised_system_baseline_fails(self):
        for old, new in [("cp38-abi3", "cp39-abi3"), ("manylinux2014", "manylinux_2_28")]:
            wheels = list(self.wheels)
            wheels[0] = Path(str(wheels[0]).replace(old, new))
            with self.subTest(replacement=new), self.assertRaisesRegex(ValueError, "unexpected"):
                check_coverage(wheels)

    def test_ci_native_and_abi3_runtime_coverage(self):
        matrices = ci_matrices()
        hosts = {"ubuntu-22.04", "macos-15-intel", "macos-15", "windows-2022"}
        native_versions = {"3.9", "3.10", "3.11", "3.12", "3.13", "3.14"}
        self.assertEqual(matrices["native"]["include"], [entry for entry in matrices["runtime"]["include"] if entry["native"]])
        self.assertEqual(len(matrices["native"]["include"]), 24)
        self.assertEqual(len(matrices["runtime"]["include"]), 28)
        self.assertEqual({(entry["os"], entry["python"]) for entry in matrices["native"]["include"]},
                         {(host, version) for host in hosts for version in native_versions})
        self.assertEqual({(entry["os"], entry["python"]) for entry in matrices["runtime"]["include"]},
                         {(host, version) for host in hosts for version in native_versions | {"3.8"}})
        self.assertEqual({entry["os"] for entry in matrices["abi3"]["include"]}, hosts)
        self.assertEqual(len(matrices["alternative"]["include"]), 13)

    def test_free_threaded_and_alternate_runtimes_never_test_abi3(self):
        matrices = ci_matrices()
        self.assertTrue(all(row["python"] in ["3.8", "3.9", "3.10", "3.11", "3.12", "3.13", "3.14"]
                            for row in matrices["runtime"]["include"]))
        self.assertEqual({row["kind"] for row in matrices["alternative"]["include"]},
                         {"pypy", "graalpy", "free-threaded"})


class BuildGroupsTest(unittest.TestCase):
    def setUp(self):
        self.entries = {entry["id"]: entry for matrix in build_matrices() for entry in matrix}
        self.groups = [group for matrix in build_groups() for group in matrix]

    def test_groups_partition_all_artifacts(self):
        ids = [artifact for group in self.groups for artifact in group["artifact_ids"]]
        self.assertEqual(Counter(ids), Counter(self.entries.keys()))
        self.assertEqual(len({group["id"] for group in self.groups}), len(self.groups))
        self.assertEqual(len(self.groups), 61)
        for group in self.groups:
            entries = [self.entries[artifact] for artifact in group["artifact_ids"]]
            for key in ("platform", "kind", "features", "target"):
                self.assertEqual({entry[key] for entry in entries}, {group[key]})
            self.assertEqual(group["interpreters"].split(), [entry["interpreter"] for entry in entries])
            if group["kind"] == "native":
                expected = ["3.11", "3.12", "3.13", "3.14"] if group["target"] == "aarch64-pc-windows-msvc" else ["3.9", "3.10", "3.11", "3.12", "3.13", "3.14"]
                self.assertEqual(group["interpreters"].split(), expected)
            else:
                self.assertEqual(len(entries), 1)

    def test_only_graalpy313_uses_new_baseline(self):
        changed = [entry for entry in self.entries.values() if entry.get("policy") == "manylinux_2_28"]
        self.assertEqual({entry["id"] for entry in changed}, {
            "manylinux_2_28-x86_64-graalpy-3.13", "manylinux_2_28-aarch64-graalpy-3.13",
        })
        baselines = Counter(entry["policy"] for entry in self.entries.values() if "policy" in entry)
        self.assertEqual(baselines, {"manylinux2014": 52, "manylinux_2_31": 8, "musllinux_1_1": 28, "manylinux_2_28": 2})

    def test_desktop_requests_keep_architecture_and_abi(self):
        for group in build_groups()[1]:
            if group["kind"] == "graalpy":
                self.assertEqual(group["python_requests"], [])
                self.assertTrue(group["setup_python"].startswith("graalpy-"))
                continue
            self.assertEqual(len(group["python_requests"]), len(group["artifact_ids"]))
            for request in group["python_requests"]:
                if group["arch"] == "x86":
                    self.assertIn("-windows-i686-none", request)
                elif group["arch"] == "arm64":
                    self.assertTrue(request.endswith("-aarch64-none"))
                else:
                    self.assertTrue(request.endswith("-x86_64-none"))
                self.assertEqual("+freethreaded" in request, group["kind"] == "free-threaded")
                if group["kind"] == "pypy":
                    self.assertTrue(request.startswith("pypy-3.11.15-"))

    def test_prepare_counts_artifacts_not_jobs(self):
        import json
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "outputs"
            with patch.dict("os.environ", {"GITHUB_OUTPUT": str(output), "DRAFT_VERSION": "", "GITHUB_EVENT_NAME": "workflow_dispatch"}):
                prepare()
            values = dict(line.split("=", 1) for line in output.read_text().splitlines())
        self.assertEqual(values["wheel_count"], "134")
        self.assertEqual(sum(len(json.loads(values[key])["include"]) for key in ("linux", "desktop")), 61)
        self.assertEqual(values["tag"], "")

    def test_desktop_selection_installs_all_and_quotes_paths(self):
        group = next(group for group in self.groups if group["id"] == "aarch64-apple-darwin-native")
        paths = ["/managed python/{}/python\n".format(i) for i in range(6)]
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "outputs"
            with patch.dict("os.environ", {"GITHUB_OUTPUT": str(output)}), patch("release.subprocess.run") as run, patch("release.subprocess.check_output", side_effect=paths) as find:
                desktop_interpreters(group["id"])
            self.assertEqual(output.read_text(), "interpreters={}\n".format(" ".join('"{}"'.format(path.strip()) for path in paths)))
        run.assert_called_once_with(["uv", "python", "install", *group["python_requests"]], check=True)
        self.assertEqual([call.args[0][-1] for call in find.call_args_list], group["python_requests"])
        self.assertTrue(all("--managed-python" in call.args[0] and "--system" in call.args[0] for call in find.call_args_list))


class GroupValidationTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.directory = Path(self.temp.name)
        self.group = "manylinux2014-x86_64-native"
        self.entries = [entry for matrix in build_matrices() for entry in matrix
                        if entry["platform"] == "manylinux2014-x86_64" and entry["kind"] == "native"]
        self.wheels = [self.write_wheel(entry) for entry in self.entries]

    def write_wheel(self, entry, version="0.1.0", metadata_version="0.1.0", tag=None):
        tag = tag or "{}-{}".format(entry["tag"], entry["wheel_platform"])
        wheel = self.directory / "aliyun_log_producer-{}-{}.whl".format(version, tag)
        with zipfile.ZipFile(wheel, "w") as archive:
            archive.writestr("aliyun_log_producer.dist-info/WHEEL", "Wheel-Version: 1.0\nTag: {}\n".format(tag))
            archive.writestr("aliyun_log_producer.dist-info/METADATA", "Name: aliyun-log-producer\nVersion: {}\nRequires-Python: >=3.8\n".format(metadata_version))
        return wheel

    def test_complete_group(self):
        check_group(self.directory, self.group, "0.1.0")

    def test_final_distribution_still_requires_all_wheels_and_audits_abi3(self):
        for matrix in build_matrices():
            for entry in matrix:
                self.write_wheel(entry)
        metadata = b"Name: aliyun-log-producer\nVersion: 0.1.0\nRequires-Python: >=3.8\n"
        with tarfile.open(self.directory / "aliyun_log_producer-0.1.0.tar.gz", "w:gz") as archive:
            info = tarfile.TarInfo("aliyun_log_producer-0.1.0/PKG-INFO")
            info.size = len(metadata)
            archive.addfile(info, io.BytesIO(metadata))
        with patch("release.subprocess.run") as audit:
            check_dist(self.directory, "0.1.0", 134)
        self.assertEqual(audit.call_count, 16)
        self.assertTrue(all("abi3audit" in call.args[0] and "--strict" in call.args[0] for call in audit.call_args_list))
        self.assertEqual(len((self.directory / "SHA256SUMS").read_text().splitlines()), 135)
        self.wheels[-1].unlink()
        with self.assertRaisesRegex(ValueError, "expected 134 wheels"):
            check_dist(self.directory, "0.1.0", 134)

    def test_missing_interpreter_is_not_success(self):
        self.wheels[-1].unlink()
        with self.assertRaisesRegex(ValueError, "missing"):
            check_group(self.directory, self.group, "0.1.0")

    def test_empty_group_is_not_success(self):
        for wheel in self.wheels:
            wheel.unlink()
        with self.assertRaisesRegex(ValueError, "missing"):
            check_group(self.directory, self.group, "0.1.0")

    def test_extra_runtime_and_wrong_baseline_are_rejected(self):
        for tag in ("cp314-cp314t-manylinux2014_x86_64", "cp314-cp314-manylinux_2_28_x86_64"):
            wheel = self.write_wheel(self.entries[-1], tag=tag)
            with self.subTest(tag=tag), self.assertRaisesRegex(ValueError, "unexpected"):
                check_group(self.directory, self.group, "0.1.0")
            wheel.unlink()

    def test_duplicate_cannot_hide_missing_interpreter(self):
        self.wheels[-1].unlink()
        self.write_wheel(self.entries[0], version="0.2.0")
        with self.assertRaisesRegex(ValueError, "duplicate"):
            check_group(self.directory, self.group, "0.1.0")

    def test_wrong_metadata_version_is_rejected(self):
        self.write_wheel(self.entries[0], metadata_version="0.2.0")
        with self.assertRaisesRegex(ValueError, "version"):
            check_group(self.directory, self.group, "0.1.0")


if __name__ == "__main__":
    unittest.main()
