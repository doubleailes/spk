from typing import List, Optional, Iterable, Union
from . import api, Digest

def local_repository() -> Repository: ...
def remote_repository(path: str = "origin") -> Repository: ...
def open_tar_repository(path: str, create: bool = False) -> Repository: ...

class Repository:
    def is_spfs(self) -> bool: ...
    def list_packages(self) -> Iterable[str]: ...
    def list_package_versions(self, name: str) -> Iterable[str]: ...
    def list_package_builds(
        self, pkg: Union[str, api.Ident]
    ) -> Iterable[api.Ident]: ...
    def read_spec(self, pkg: api.Ident) -> api.Spec: ...
    def get_package(self, pkg: api.Ident) -> Digest: ...
    def publish_spec(self, spec: api.Spec) -> None: ...
    def remove_spec(self, pkg: api.Ident) -> None: ...
    def force_publish_spec(self, spec: api.Spec) -> None: ...
    def publish_package(self, spec: api.Spec, digest: Digest) -> None: ...
    def remove_package(self, pkg: api.Ident) -> None: ...
