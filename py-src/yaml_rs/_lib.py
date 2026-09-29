import enum
from pathlib import Path
from typing import Any, BinaryIO, Literal, TextIO, final

from ._yaml_rs import (
    _AliasLimits as AliasLimits,
    _dumps,
    _load,
    _loads,
)


@final
@enum.unique
class DuplicateKeyPolicy(enum.StrEnum):
    Error = "error"
    FirstWins = "first_wins"
    LastWins = "last_wins"


def load(
    file_obj: BinaryIO | bytes | str,
    /,
    *,
    parse_datetime: bool = True,
    encoding: str | None = None,
    encoder_errors: Literal["ignore", "replace", "strict"] | None = None,
    alias_limits: AliasLimits | None = None,
    duplicate_key_policy: DuplicateKeyPolicy | None = DuplicateKeyPolicy.Error,
) -> dict[str, Any] | list[dict[str, Any]]:
    return _load(
        file_obj,
        parse_datetime=parse_datetime,
        encoding=encoding,
        encoder_errors=encoder_errors,
        alias_limits=alias_limits,
        duplicate_key_policy=duplicate_key_policy,
    )


def loads(
    str_obj: str,
    /,
    *,
    parse_datetime: bool = True,
    alias_limits: AliasLimits | None = None,
    duplicate_key_policy: DuplicateKeyPolicy | None = DuplicateKeyPolicy.Error,
) -> dict[str, Any] | list[dict[str, Any]]:
    if not isinstance(str_obj, str):
        msg = f"Expected str object, not '{type(str_obj).__qualname__}'"
        raise TypeError(msg)

    return _loads(
        str_obj,
        parse_datetime=parse_datetime,
        alias_limits=alias_limits,
        duplicate_key_policy=duplicate_key_policy,
    )


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
