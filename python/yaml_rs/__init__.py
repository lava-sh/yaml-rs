__all__ = (
    "YAMLDecodeError",
    "YAMLEncodeError",
    "__version__",
    "dump",
    "dumps",
    "load",
    "loads",
)

from ._lib import (
    __version__,
    dump,
    dumps,
    load,
    loads,
)
from ._yaml_rs import (
    YAMLDecodeError,
    YAMLEncodeError,
)
