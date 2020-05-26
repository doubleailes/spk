from typing import Union, Optional
from dataclasses import dataclass, field

from ruamel import yaml

from .. import compat
from ._build import Build, parse_build


@dataclass
class Ident:
    """Ident represents a package identifier.

	The identifier is either a specific package or
	range of package versions/releases depending on the
	syntax and context
	"""

    name: str
    version: compat.Version = field(default_factory=lambda: compat.Version(""))
    build: Optional[Build] = None

    def __str__(self) -> str:

        version = str(self.version)
        out = self.name
        if version:
            out += "/" + version
        if self.build:
            out += "/" + self.build.digest
        return out

    __repr__ = __str__

    def copy(self) -> "Ident":
        """Create a copy of this identifier."""

        return parse_ident(str(self))

    def with_build(self, build: Union[Build, str]) -> "Ident":
        """Return a copy of this identifier with the given build replaced"""

        return parse_ident(f"{self.name}/{self.version}/{build}")

    def parse(self, source: str) -> None:
        """Parse the given didentifier string into this instance.
        """

        name, version, build, *other = str(source).split("/") + ["", ""]

        if any(other):
            raise ValueError(f"Too many tokens in identifier: {source}")

        self.name = name
        self.version = compat.parse_version(version)
        self.build = parse_build(build) if build else None


def parse_ident(source: str) -> Ident:
    """Parse a package identifier string."""
    ident = Ident("")
    ident.parse(source)
    return ident


yaml.Dumper.add_representer(
    Ident,
    lambda dumper, data: yaml.representer.SafeRepresenter.represent_str(
        dumper, str(data)
    ),
)

yaml.SafeDumper.add_representer(
    Ident,
    lambda dumper, data: yaml.representer.SafeRepresenter.represent_str(
        dumper, str(data)
    ),
)