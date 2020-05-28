from typing import List, Dict, Set, Optional, Union, Iterator, Tuple, Iterable
from collections import defaultdict
from functools import lru_cache

from .. import api, storage, compat
from ._errors import SolverError, ConflictingRequestsError


class PackageIterator(Iterator[Tuple[api.Ident, api.Spec]]):
    """PackageIterator is a stateful cursor yielding package versions.

    The iterator yields packages from a repository which are compatible with some
    request. These are used to retain a cursor in the repo in the case of
    needing to continue with next-best option upon error or issue in the solve.
    """

    def __init__(
        self, repo: storage.Repository, request: api.Ident, options: api.OptionMap
    ) -> None:
        self._repo = repo
        self._request = request
        self._options = options
        self._versions: Optional[Iterator[str]] = None
        self.past_versions: List[str] = []

    def _start(self) -> None:
        all_versions = self._repo.list_package_versions(self._request.name)
        versions = list(filter(self._request.version.is_satisfied_by, all_versions))
        versions.sort()
        versions.reverse()
        self._versions = iter(versions)

    def clone(self) -> "PackageIterator":
        """Create a copy of this iterator, with the cursor at the same point."""

        if self._versions is None:
            self._start()

        other = PackageIterator(self._repo, self._request, self._options)
        remaining = list(self._versions)  # type: ignore
        other._versions = iter(remaining)
        self._versions = iter(remaining)
        return other

    def __next__(self) -> Tuple[api.Ident, api.Spec]:

        if self._versions is None:
            self._start()

        for version_str in self._versions:  # type: ignore
            self.past_versions.append(version_str)
            version = compat.parse_version(version_str)
            pkg = api.Ident(self._request.name, version)
            spec = self._repo.read_spec(pkg)
            options = spec.resolve_all_options(self._options)

            candidate = pkg.with_build(options.digest())
            try:
                self._repo.get_package(candidate)
            except storage.PackageNotFoundError:
                continue

            return (candidate, spec)

        raise StopIteration


class Decision:
    """Decision represents a change in state for the solver.

    Decisions form a tree structure. Each decision can have a single
    parent, and any number of child branches which should represent
    different possible subsequent decisions made by the solver.

    The root decision in the tree will not have a parent, and generally
    holds the set of initial requests for a resolve.

    Decisions provide the state of the resolve after this decision has been
    applied by merging the decision changes with all parents.

    Decisions usually resolve a single package request and optionally
    add additional requests (from depenencies). If a dependency
    is added which invalidates a previously resolved package, they
    can also 'reopen/unresolve' a package to be resolved again.
    If some unrecoverable issue caused the solver's not to be able to
    continue from the parent state, decision.get_error() will return
    the relevant exception.
    """

    def __init__(self, parent: "Decision" = None) -> None:
        self.parent = parent
        self.branches: List[Decision] = []
        self._requests: Dict[str, List[api.Ident]] = defaultdict(list)
        self._resolved: Dict[str, api.Ident] = {}
        self._unresolved: Set[str] = set()
        self._error: Optional[SolverError] = None
        self._iterators: Dict[str, PackageIterator] = {}

    def __str__(self) -> str:
        if self._error is not None:
            return f"STOP: {self._error}"
        out = ""
        if self._resolved:
            values = list(str(pkg) for pkg in self._resolved.values())
            out += f"RESOLVE: {', '.join(values)} "
        if self._requests:
            values = list(str(pkg) for pkg in self._requests.values())
            out += f"REQUEST: {', '.join(values)} "
        return out

    @lru_cache()
    def level(self) -> int:
        """Return the depth of this decision in it's tree (number or parents)."""

        level = 0
        parent = self.parent
        while parent is not None:
            level += 1
            parent = parent.parent
        return level

    def set_error(self, error: SolverError) -> None:
        """Set the error on this decision, marking it as an invalid state."""

        self._error = error

    def get_error(self) -> Optional[SolverError]:
        """Get the error caused by this decision (if any)."""
        return self._error

    def set_resolved(self, pkg: api.Ident) -> None:
        """Set the given package as resolved by this decision.

        The given identifier is expected to be a fully resolved package with exact build.
        """

        self.unresolved_requests.cache_clear()
        self._resolved[pkg.name] = pkg

    def get_resolved(self) -> Dict[str, api.Ident]:
        """Get the set of packages resolved by this decision."""

        return dict((n, pkg.clone()) for n, pkg in self._resolved.items())

    def set_unresolved(self, pkg: api.Ident) -> None:
        """Set the given package as unresolved by this decision.

        An unresolved package undoes any previous decision that resolves
        the package and forces the solver to resolve it again.

        This usually only makes sense if the decision introduces a new
        request which is not satisfied by the previous resolve, and will
        be called automatically in this case when Decision.add_request is called
        """

        self.unresolved_requests.cache_clear()
        self._unresolved.add(pkg.name)

    def get_unresolved(self) -> List[str]:
        """Get the set of packages that are unresolved by this decision."""

        return list(self._unresolved)

    def get_iterator(self, name: str) -> Optional[PackageIterator]:
        """Get the current package iterator for this state.

        The returned iterator, if not none, will iterate through any remaining
        versions of the named package that are compatible with the solver
        state represented by this decision
        """

        if name not in self._iterators:
            if self.parent is not None:
                parent_iter = self.parent.get_iterator(name)
                if parent_iter is not None:
                    self._iterators[name] = parent_iter.clone()

        return self._iterators.get(name)

    def set_iterator(self, name: str, iterator: PackageIterator) -> None:
        """Set a package iterator for this decision.

        The given iterator represents the available package verisons
        compatible with the solver state represented by this decision.
        """

        self._iterators[name] = iterator

    def add_request(self, pkg: Union[str, api.Ident]) -> None:
        """Add a package request to this decision

        This request may be a new package, or a new constraint on an existing
        requested package. Either way the solver will ensure it is satisfied
        should this decision branch be deemed solvable.
        """

        if not isinstance(pkg, api.Ident):
            pkg = api.parse_ident(pkg)

        current = self.get_current_packages().get(pkg.name)
        if current is not None:
            if not pkg.version.is_satisfied_by(current.version):
                self.set_unresolved(pkg)
        else:
            self.unresolved_requests.cache_clear()

        self._requests[pkg.name].append(pkg)

    def get_requests(self) -> Dict[str, List[api.Ident]]:
        """Get the set of package requests added by this decision."""

        copy = {}
        for name, reqs in self._requests.items():
            copy[name] = list(pkg.clone() for pkg in reqs)
        return copy

    def add_branch(self) -> "Decision":
        """Add a child branch to this decision."""

        branch = Decision(self)
        self.branches.append(branch)
        return branch

    def get_current_packages(self) -> Dict[str, api.Ident]:
        """Get the full set of resolved packages for this decision state

        Unlike get_resolved, this includes resolved packages from all parents.
        """

        packages = {}
        if self.parent is not None:
            packages.update(self.parent.get_current_packages())
        packages.update(self._resolved)

        for name in self._unresolved:
            if name in packages:
                del packages[name]

        return packages

    def has_unresolved_requests(self) -> bool:
        """Return true if there are unsatisfied package requests in this solver state."""

        return len(self.unresolved_requests()) != 0

    def next_request(self) -> Optional[api.Ident]:
        """Return the next package request to be resolved in this state."""

        unresolved = self.unresolved_requests()
        if len(unresolved) == 0:
            return None

        return self.get_merged_request(next(iter(unresolved.keys())))

    @lru_cache()
    def unresolved_requests(self) -> Dict[str, List[api.Ident]]:
        """Return the complete set of unresolved requests for this solver state."""

        resolved = self.get_current_packages()
        requests = self.get_all_package_requests()

        unresolved = dict((n, r) for n, r in requests.items() if n not in resolved)
        return unresolved

    def get_all_package_requests(self) -> Dict[str, List[api.Ident]]:
        """Get the set of all package requests at this state, solved or not."""

        base: Dict[str, List[api.Ident]] = defaultdict(list)
        if self.parent is not None:
            base.update(self.parent.get_all_package_requests())

        for name in self._requests:
            base[name].extend(self._requests[name])

        return base

    def get_package_requests(self, name: str) -> List[api.Ident]:
        """Get the set of requests in this state for the named package."""

        requests = []
        if self.parent is not None:
            requests.extend(self.parent.get_package_requests(name))
        requests.extend(self._requests[name])
        return requests

    def get_merged_request(self, name: str) -> Optional[api.Ident]:
        """Get a single request for the named package which satisfies all current requests for that package."""

        requests = self.get_package_requests(name)

        if not requests:
            return None

        merged = requests[0].clone()
        for request in requests[1:]:
            try:
                merged.restrict(request)
            except ValueError as e:
                raise ConflictingRequestsError(str(e), requests)

        return merged


class DecisionTree:
    """Decision tree represents an entire set of solver decisions

    The decision tree provides convenience methods for working
    with an entire decision tree at once.
    """

    def __init__(self) -> None:

        self.root = Decision()

    def walk(self) -> Iterable[Decision]:

        to_walk = [self.root]
        while len(to_walk):
            here = to_walk.pop()
            yield here
            to_walk.extend(reversed(here.branches))
