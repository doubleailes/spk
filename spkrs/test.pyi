from typing import List, Sequence

from . import solve, api, storage

class TestError(Exception):
    pass

class PackageSourceTester:
    def __init__(self, spec: api.Spec, script: str) -> None: ...
    def get_solve_graph(self) -> solve.Graph: ...
    def with_option(self, name: str, value: str) -> PackageSourceTester: ...
    def with_options(self, options: api.OptionMap) -> PackageSourceTester: ...
    def with_repository(self, repo: storage.Repository) -> PackageSourceTester: ...
    def with_repositories(
        self, repos: List[storage.Repository]
    ) -> PackageSourceTester: ...
    def with_source(self, source: str) -> PackageSourceTester: ...
    def with_requirements(
        self, requests: Sequence[api.Request]
    ) -> PackageSourceTester: ...
    def test(self) -> None: ...

class PackageInstallTester:
    pass

class PackageBuildTester:
    pass
