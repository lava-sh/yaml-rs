from pathlib import Path
from typing import Any, BinaryIO, Literal, TextIO

from ._yaml_rs import (
    _VERSION,
    _dumps,
    _load,
    _loads,
)

__version__: str = _VERSION


def load(
    fp: BinaryIO | bytes | str,
    /,
    *,
    parse_datetime: bool = True,
    encoding: str | None = None,
    encoder_errors: Literal["ignore", "replace", "strict"] | None = None,
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
        msg = f"Expected str object, not '{type(s).__qualname__}'"
        raise TypeError(msg)
    return _loads(s, parse_datetime=parse_datetime)


def dump(
    obj: Any,
    /,
    file: str | Path | TextIO,
    *,
    compact: bool = True,
    multiline_strings: bool = True,
) -> int:
    toml_str = _dumps(obj, compact=compact, multiline_strings=multiline_strings)
    if isinstance(file, str):
        file = Path(file)
    if isinstance(file, Path):
        return file.write_text(toml_str, encoding="utf-8")

    return file.write(toml_str)


def dumps(
    obj: Any,
    /,
    *,
    compact: bool = True,
    multiline_strings: bool = True,
) -> str:
    return _dumps(obj, compact=compact, multiline_strings=multiline_strings)
