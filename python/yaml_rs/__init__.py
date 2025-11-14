__all__ = (
    "YAMLDecodeError",
    "YAMLEncodeError",
    "__version__",
    "dump",
    "dumps",
    "load",
    "loads",
)

from pathlib import Path
from typing import Any, BinaryIO, TextIO

from ._yaml_rs import (
    YAMLDecodeError,
    YAMLEncodeError,
    _dumps,
    _loads,
    _version,
)

__version__: str = _version


def load(
    fp: BinaryIO,
    /,
    *,
    parse_datetime: bool = True,
) -> dict[str, Any] | list[dict[str, Any]]:
    _bytes = fp.read()
    try:
        _str = _bytes.decode()
    except AttributeError:
        msg = "File must be opened in binary mode, e.g. use `open('config.yaml', 'rb')`"
        raise TypeError(msg) from None
    return loads(_str, parse_datetime=parse_datetime)


def loads(
    s: str,
    /,
    *,
    parse_datetime: bool = True,
) -> dict[str, Any] | list[dict[str, Any]]:
    if not isinstance(s, str):
        raise TypeError(f"Expected str object, not '{type(s).__name__}'")
    return _loads(s, parse_datetime=parse_datetime)


def dump(obj: Any, /, file: str | Path | TextIO) -> int:
    _str = _dumps(obj)
    if isinstance(file, str):
        file = Path(file)
    if isinstance(file, Path):
        return file.write_text(_str, encoding="utf-8")
    else:
        return file.write(_str)


def dumps(obj: Any, /) -> str:
    return _dumps(obj)
