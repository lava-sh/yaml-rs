__all__ = (
    "AliasLimits",
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
    dump,
    dumps,
    load,
    loads,
)
from ._yaml_rs import (
    _VERSION as __version__,  # noqa: N811
    YAMLDecodeError,
    YAMLEncodeError,
)
