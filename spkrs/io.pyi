from typing import Sequence, Iterable, Tuple, TextIO
from . import api, solve

def format_ident(pkg: api.Ident) -> str: ...
def format_build(build: str) -> str: ...
def format_options(options: api.OptionMap) -> str: ...
def format_request(name: str, requests: Sequence[api.Request]) -> str: ...
def format_solution(solution: solve.Solution, verbosity: int = 0) -> str: ...
def format_note(note: solve.graph.Note) -> str: ...
def change_is_relevant_at_verbosity(change: solve.graph.Change, verbosity: int) -> bool: ...
def format_change(change: solve.graph.Change, verbosity: int = 1) -> str: ...
def format_decisions(
    decisions: Iterable[Tuple[solve.graph.Node, solve.graph.Decision]],
    verbosity: int = 1,
) -> str: ...
def print_decisions(
    decisions: Iterable[Tuple[solve.graph.Node, solve.graph.Decision]],
    verbosity: int = 1,
) -> None: ...
