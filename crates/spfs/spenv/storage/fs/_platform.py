from typing import Optional, List, Dict, NamedTuple, Sequence
import os
import json
import uuid
import errno
import shutil
import hashlib

import structlog

_logger = structlog.get_logger(__name__)

from .. import Platform
from ._layer import Layer


class PlatformStorage:
    def __init__(self, root: str) -> None:

        self._root = os.path.abspath(root)

    def read_platform(self, digest: str) -> Platform:
        """Read a platform's information from this storage.

        Raises:
            ValueError: If the platform does not exist.
        """

        platform_path = os.path.join(self._root, digest)
        try:
            with open(platform_path, "r", encoding="utf-8") as f:
                data = json.load(f)
            return Platform.load_dict(data)
        except OSError as e:
            if e.errno == errno.ENOENT:
                raise ValueError(f"Unknown platform: {digest}")
            raise

    def remove_platform(self, digest: str) -> None:
        """Remove a platform from this storage.

        Raises:
            ValueError: If the platform does not exist.
        """

        platform_path = os.path.join(self._root, digest)
        try:
            os.remove(platform_path)
        except FileNotFoundError:
            raise ValueError(f"Unknown platform: {digest}")

    def list_platforms(self) -> List[Platform]:
        """Return a list of the current stored platforms."""

        try:
            dirs = os.listdir(self._root)
        except OSError as e:
            if e.errno == errno.ENOENT:
                dirs = []
            else:
                raise

        return [self.read_platform(d) for d in dirs]

    def commit_stack(self, stack: Sequence[str]) -> Platform:

        platform = Platform(stack=tuple(stack))
        self.write_platform(platform)
        return platform

    def write_platform(self, platform: Platform) -> None:
        """Store the given platform data in this storage."""

        digest = platform.digest
        platform_path = os.path.join(self._root, digest)
        os.makedirs(self._root, exist_ok=True)
        try:
            with open(platform_path, "x", encoding="utf-8") as f:
                json.dump(platform.dump_dict(), f)
            _logger.debug("platform created", digest=digest)
        except FileExistsError:
            _logger.debug("platform already exists", digest=digest)
