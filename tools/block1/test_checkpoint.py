"""Unit tests for the B1 private checkpoint coordinator (synthetic CTF bytes only)."""
from __future__ import annotations
import hashlib
import json
import pathlib
import subprocess
import sys
import tempfile
import unittest

ROOT=pathlib.Path(__file__).resolve().parents[2]
SCRIPT=ROOT/"tools/block1/run_private_ctf.py"

def fake_executable(path:pathlib.Path,body:str)->pathlib.Path:
    path.write_text("#!/usr/bin/env python3\n"+body,encoding="utf-8")
    path.chmod(0o700)
    return path

class CheckpointTest(unittest.TestCase):
    def fixture(self,root:pathlib.Path):
        first=root/"a.CTF";second=root/"b.CTF"
        first.write_bytes(b"secret example record a")
        second.write_bytes(b"secret example record b")
        return first,second

    def invoke(self,root,first,second,probe,acceptance,output):
        return subprocess.run(
            [sys.executable,str(SCRIPT),"--probe",str(probe),
             "--acceptance",str(acceptance),"--output",str(output),
             str(first),str(second)],
            cwd=root,capture_output=True,text=True,check=False
        )

    def test_success_preserves_input_and_refuses_checkpoint_overwrite(self):
        with tempfile.TemporaryDirectory(prefix="b1-tests-") as directory:
            root=pathlib.Path(directory);a,b=self.fixture(root)
            p=fake_executable(root/"probe",'''import json
print(json.dumps({"datasets":[{"report":{"status":"parsed","post_check":{"source_unchanged":True}}}]*2}))
''')
            c=fake_executable(root/"acceptance",'''import json
print(json.dumps({"reports":[{"status":"passed_available_checks"}]*2}))
''')
            dest=root/"checkpoint"
            result=self.invoke(root,a,b,p,c,dest)
            self.assertEqual(result.returncode,0,result.stderr)
            manifest=json.loads((dest/"B1_CHECKPOINT.json").read_text())
            self.assertTrue(manifest["passed_available_checks"])
            self.assertEqual(manifest["files"][0]["sha256_before"],
                             hashlib.sha256(a.read_bytes()).hexdigest())
            self.assertTrue(all(x["unchanged"] for x in manifest["files"]))
            self.assertTrue((dest/"SHA256SUMS.txt").exists())
            result=self.invoke(root,a,b,p,c,dest)
            self.assertNotEqual(result.returncode,0)
            self.assertTrue(manifest["passed_available_checks"])

    def test_external_edit_fails_closed_even_with_successful_subcommands(self):
        with tempfile.TemporaryDirectory(prefix="b1-tests-") as directory:
            root=pathlib.Path(directory);a,b=self.fixture(root)
            p=fake_executable(root/"probe",'''import json
print(json.dumps({"datasets":[{"report":{"status":"parsed","post_check":{"source_unchanged":True}}}]*2}))
''')
            c=fake_executable(root/"acceptance",'''import json,sys,pathlib
pathlib.Path(sys.argv[1]).write_bytes(b"external edit to temporary test fixture")
print(json.dumps({"reports":[{"status":"passed_available_checks"}]*2}))
''')
            dest=root/"checkpoint"
            result=self.invoke(root,a,b,p,c,dest)
            self.assertNotEqual(result.returncode,0)
            manifest=json.loads((dest/"B1_CHECKPOINT.json").read_text())
            self.assertFalse(manifest["passed_available_checks"])
            self.assertFalse(manifest["files"][0]["unchanged"])
            self.assertTrue(manifest["files"][1]["unchanged"])

    def test_partial_failures_still_write_checkpoint(self):
        with tempfile.TemporaryDirectory(prefix="b1-tests-") as directory:
            root=pathlib.Path(directory);a,b=self.fixture(root)
            p=fake_executable(root/"probe",'''import json
print(json.dumps({"datasets":[{"report":{"status":"parser_failed"}}]*2}))
''')
            c=fake_executable(root/"acceptance",'''import sys
sys.exit(9)
''')
            dest=root/"checkpoint"
            result=self.invoke(root,a,b,p,c,dest)
            self.assertEqual(result.returncode,1)
            manifest=json.loads((dest/"B1_CHECKPOINT.json").read_text())
            self.assertFalse(manifest["passed_available_checks"])
            self.assertEqual(len(manifest["phases"]),2)
            self.assertEqual(manifest["phases"][0]["status"],"failed")
            self.assertEqual(manifest["phases"][1]["status"],"failed")

if __name__=="__main__":
    unittest.main()
