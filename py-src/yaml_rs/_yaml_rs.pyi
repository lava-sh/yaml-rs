from typing import Any, BinaryIO, Literal

_VERSION: str

def _dumps(
    obj: Any,
    /,
    *,
    compact: bool = True,
    multiline_strings: bool = True,
) -> str: ...

def _loads(
    s: str,
    /,
    *,
    parse_datetime: bool = True,
) -> dict[str, Any] | list[dict[str, Any]]: ...

def _load(
    fp: BinaryIO | bytes | str,
    /,
    *,
    parse_datetime: bool = True,
    encoding: str | None = None,
    encoder_errors: Literal["ignore", "replace", "strict"] | None = None,
) -> dict[str, Any] | list[dict[str, Any]]: ...

class YAMLDecodeError(ValueError): ...
class YAMLEncodeError(TypeError): ...
