import math
from collections.abc import Iterable
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from dirty_equals import IsFloatNan

# https://github.com/yaml/yaml-test-suite/releases/tag/data-2022-01-17
YAML_TEST_SUITE = Path(__file__).resolve().parent / "yaml-test-suite"

ALL_YAMLS = 402


@dataclass(slots=True, frozen=True)
class YamlTestSuite:
    id: str
    dir: Path
    in_yaml: Path
    out_yaml: Path | None
    in_json: Path | None
    is_error: bool


def iter_yaml_test_suite(root: Path) -> Iterable[YamlTestSuite]:
    root = root.resolve()

    for in_yaml in root.rglob("in.yaml"):
        dir_ = in_yaml.parent

        in_json = dir_ / "in.json"
        out_yaml = dir_ / "out.yaml"
        err = (dir_ / "error").exists()

        rel = dir_.relative_to(root).as_posix()

        yield YamlTestSuite(
            id=rel,
            dir=dir_,
            in_yaml=in_yaml,
            out_yaml=out_yaml if out_yaml.exists() else None,
            in_json=in_json if in_json.exists() else None,
            is_error=err,
        )


def split_cases(cases: Iterable[YamlTestSuite]) -> tuple:
    valid = []
    invalid = []
    skipped = []

    for ts in cases:
        if ts.is_error:
            invalid.append(ts)
        elif ts.in_json is None:
            skipped.append(ts)
        else:
            valid.append(ts)

    return valid, invalid, skipped


YAML_FILES = list(iter_yaml_test_suite(YAML_TEST_SUITE))
VALID_YAMLS, INVALID_YAMLS, SKIPPED_YAMLS = split_cases(YAML_FILES)

assert (
    len(YAML_FILES)
    == len(VALID_YAMLS) + len(INVALID_YAMLS) + len(SKIPPED_YAMLS)
    == ALL_YAMLS
)


def _is_nan(obj: Any) -> Any | dict[Any, Any] | list[Any] | IsFloatNan:
    if isinstance(obj, dict):
        return {k: _is_nan(v) for k, v in obj.items()}
    if isinstance(obj, list):
        return [_is_nan(v) for v in obj]
    if isinstance(obj, float) and math.isnan(obj):
        return IsFloatNan
    return obj
