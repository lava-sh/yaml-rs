__all__ = (
    "INVALID_YAMLS",
    "VALID_YAMLS",
    "YAML_FILES",
    "_is_nan",
    "normalize_yaml",
)

import math
from pathlib import Path
from typing import Any

import yaml_rs
from dirty_equals import IsFloatNan

# https://github.com/yaml/yaml-test-suite
YAML_TEST_SUITE = Path(__file__).resolve().parent / "data" / "yaml-test-suite"
YAML_FILES = list(YAML_TEST_SUITE.glob("*.yaml"))

ALL_YAMLS = 351


def _get_yamls():
    valid = []
    invalid = []
    skipped = []

    for yaml_file in YAML_FILES:
        docs = yaml_rs.loads(yaml_file.read_text(encoding="utf-8"), parse_datetime=False)
        docs = [docs] if isinstance(docs, dict) else docs

        has_fail = any(doc.get("fail", False) for doc in docs)
        has_skip = any(doc.get("skip", False) for doc in docs)

        if has_skip:
            skipped.append(yaml_file)
        elif has_fail:
            invalid.append(yaml_file)
        else:
            valid.append(yaml_file)

    return valid, invalid, skipped


VALID_YAMLS, INVALID_YAMLS, SKIPPED_YAMLS = _get_yamls()
assert (
    len(YAML_FILES)
    == len(VALID_YAMLS) + len(INVALID_YAMLS) + len(SKIPPED_YAMLS)
    == ALL_YAMLS
)
print(  # noqa: T201
    f"valid: {len(VALID_YAMLS)}\n"
    f"invalid: {len(INVALID_YAMLS)}\n"
    f"skipped: {len(SKIPPED_YAMLS)}\n",
)


def _is_nan(obj: Any) -> Any | dict[Any, Any] | list[Any] | IsFloatNan:
    if isinstance(obj, dict):
        return {k: _is_nan(v) for k, v in obj.items()}
    if isinstance(obj, list):
        return [_is_nan(v) for v in obj]
    if isinstance(obj, float) and math.isnan(obj):
        return IsFloatNan
    return obj


def normalize_yaml(doc: dict) -> Any:
    return (
        doc.get("yaml")
        .replace("␣", " ")
        .replace("»", "\t")
        .replace("—", "")  # Tab line continuation ——»
        .replace("←", "\r")
        .replace("⇔", "\ufeff")  # BOM character
        .replace("↵", "")  # Trailing newline marker
        .replace("∎\n", "")
    )
