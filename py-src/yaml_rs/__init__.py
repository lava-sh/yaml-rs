__all__ = (
    "AliasLimits",
    "DuplicateKeyPolicy",
    "YAMLDecodeError",
    "YAMLEncodeError",
    "__version__",
    "dump",
    "dumps",
    "load",
    "loads",
)

from ._lib import (
    AliasLimits,
    DuplicateKeyPolicy,
    dump,
    dumps,
    load,
    loads,
)
from ._yaml_rs import (
    YAMLDecodeError,
    YAMLEncodeError,
    __version__,
)
