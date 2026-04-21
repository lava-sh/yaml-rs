from typing import Any, BinaryIO, Literal

_VERSION: str

class _AliasLimits:
    max_total_replayed_events: int
    max_replay_stack_depth: int
    max_alias_expansions_per_anchor: int

    def __init__(
        self,
        max_total_replayed_events: int = 1_000_000,
        max_replay_stack_depth: int = 64,
        max_alias_expansions_per_anchor: int | None = None,
    ) -> None: ...

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
    alias_limits: _AliasLimits | None = None,
    duplicate_key_policy: Literal["error", "first_wins", "last_wins"] | None = None,
) -> dict[str, Any] | list[dict[str, Any]]: ...

def _load(
    fp: BinaryIO | bytes | str,
    /,
    *,
    parse_datetime: bool = True,
    encoding: str | None = None,
    encoder_errors: Literal["ignore", "replace", "strict"] | None = None,
    alias_limits: _AliasLimits | None = None,
    duplicate_key_policy: Literal["error", "first_wins", "last_wins"] | None = None,
) -> dict[str, Any] | list[dict[str, Any]]: ...

class YAMLDecodeError(ValueError): ...
class YAMLEncodeError(TypeError): ...
