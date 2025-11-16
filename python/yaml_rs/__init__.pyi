from pathlib import Path
from typing import Any, BinaryIO, TextIO

__version__: str

def load(
    fp: BinaryIO | bytes | str,
    /,
    *,
    parse_datetime: bool = True,
    encoding: str | None = None,
    encoder_errors: str | None = None,
) -> dict[str, Any]: ...
def loads(
    s: str | bytes | BinaryIO,
    /,
    *,
    parse_datetime: bool = True,
    encoding: str | None = None,
    encoder_errors: str | None = None,
) -> dict[str, Any]: ...
def dump(obj: Any, file: str | Path | TextIO) -> int: ...
def dumps(obj: Any) -> str: ...

class YAMLDecodeError(ValueError): ...
class YAMLEncodeError(TypeError): ...
