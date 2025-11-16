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
    _load,
    _loads,
    _version,
)

__version__: str = _version


def load(
    fp: BinaryIO | bytes | str,
    /,
    *,
    parse_datetime: bool = True,
    encoding: str | None = None,
    encoder_errors: str | None = None,
) -> dict[str, Any] | list[dict[str, Any]]:
    return _load(
        fp,
        parse_datetime=parse_datetime,
        encoding=encoding,
        encoder_errors=encoder_errors,
    )


def loads(
    s: str,
    /,
    *,
    parse_datetime: bool = True,
) -> dict[str, Any] | list[dict[str, Any]]:
    if not isinstance(s, str):
        raise TypeError(f"Expected str object, not '{type(s).__qualname__}'")
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
