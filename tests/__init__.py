__all__ = (
    "INVALID_YAMLS",
    "VALID_YAMLS",
    "YAML_FILES",
    "_isnan",
)

import math
from pathlib import Path

import yaml_rs
from dirty_equals import IsFloatNan

# https://github.com/yaml/yaml-test-suite
YAML_TEST_SUITE = Path(__file__).resolve().parent / "data" / "yaml-test-suite"
YAML_FILES = list(YAML_TEST_SUITE.glob("*.yaml"))


def _get_yamls():
    valid = []
    invalid = []

    for yaml_file in YAML_FILES:
        load_from_str = yaml_rs.loads(
            yaml_file.read_text(encoding="utf-8"),
            parse_datetime=False,
        )

        if isinstance(load_from_str, dict):
            docs = [load_from_str]
        elif isinstance(load_from_str, list):
            docs = load_from_str
        else:
            continue

        if any(
            doc.get("fail") or
            "Invalid" in doc.get("name", "") or
            not doc.get("json")
            for doc in docs
            if isinstance(doc, dict)
        ):
            invalid.append(yaml_file)
        else:
            valid.append(yaml_file)

    return valid, invalid


VALID_YAMLS, INVALID_YAMLS = _get_yamls()
assert len(YAML_FILES) == len(VALID_YAMLS) + len(INVALID_YAMLS)


def _isnan(obj):
    if isinstance(obj, dict):
        return {k: _isnan(v) for k, v in obj.items()}
    if isinstance(obj, list):
        return [_isnan(v) for v in obj]
    if isinstance(obj, float) and math.isnan(obj):
        return IsFloatNan
    return obj
