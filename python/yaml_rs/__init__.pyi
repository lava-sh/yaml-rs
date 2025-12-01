from pathlib import Path
from typing import Any, BinaryIO, Literal, TextIO

__version__: str

def load(
    fp: BinaryIO | bytes | str,
    /,
    *,
    parse_datetime: bool = True,
    encoding: str | None = None,
    encoder_errors: Literal["ignore", "replace", "strict"] | None = None,
) -> dict[str, Any] | list[dict[str, Any]]: ...

def loads(
    s: str,
    /,
    *,
    parse_datetime: bool = True,
) -> dict[str, Any] | list[dict[str, Any]]: ...

def dump(
    obj: Any,
    /,
    file: str | Path | TextIO,
    *,
    compact: bool = True,
    multiline_strings: bool = False,
) -> int: ...

def dumps(
    obj: Any,
    /,
    *,
    compact: bool = True,
    multiline_strings: bool = False,
) -> str: ...

class YAMLDecodeError(ValueError): ...
class YAMLEncodeError(TypeError): ...
