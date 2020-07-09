"""SPack - an SpFS Package Manager"""

__version__ = "0.7.4"

from . import api, storage, solve, build, exec
from ._global import load_spec, save_spec

# promote useful front line api functions
from .solve import (
    Solver,
    UnresolvedPackageError,
    ConflictingRequestsError,
    SolverError,
    DecisionTree,
    Decision,
)
from .api import read_spec_file
from .build import (
    SourcePackageBuilder,
    BinaryPackageBuilder,
    BuildError,
    CollectionError,
)
from .storage import export_package, import_package
from .exec import setup_current_runtime, create_runtime
