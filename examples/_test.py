from typing import Any, Iterable
import os
import sys
import subprocess
import glob
import tempfile
import itertools

import py.path
import pytest

import spkrs
import spk.cli

here = os.path.dirname(__file__)
testable_examples = glob.glob(f"{here}/**/*.spk.yaml", recursive=True)


@pytest.fixture(autouse=True, scope="session")
def tmpspfs() -> Iterable[spk.storage.SpFSRepository]:

    tmpdir = py.path.local(tempfile.mkdtemp())
    root = tmpdir.join("spfs_repo").strpath
    os.environ["SPFS_STORAGE_ROOT"] = root
    if "SPFS_REMOTE_ORIGIN_ADDRESS" in os.environ:
        del os.environ["SPFS_REMOTE_ORIGIN_ADDRESS"]
    r = py.path.local(root)
    r.join("runtimes").ensure(dir=True)
    r.join("renders").ensure(dir=True)
    r.join("objects").ensure(dir=True)
    r.join("payloads").ensure(dir=True)
    r.join("tags").ensure(dir=True)
    yield spk.storage.SpFSRepository(spkrs.SpFSRepository("file:" + root))
    tmpdir.remove(rec=1)


@pytest.mark.parametrize(
    "stage,spec_file", itertools.product(("mks", "mkb", "test"), testable_examples)
)
def test_example(stage: str, spec_file: str) -> None:

    subprocess.check_call(
        [
            os.path.dirname(sys.executable) + "/spk",
            stage,
            "-vvv",
            spec_file,
        ]
    )
