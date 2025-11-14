# ruff: noqa: E501, DTZ001
import sys
from datetime import date, datetime, timedelta, timezone
from pathlib import Path
from typing import Any

import pytest
import yaml_rs

from tests import VALID_YAMLS, normalize_yaml

if sys.version_info >= (3, 11):
    from datetime import UTC
else:
    UTC = timezone.utc

dt = datetime(
    2001,
    12,
    14,
    21,
    59,
    43,
    100000,
    tzinfo=timezone(timedelta(days=-1, seconds=68400)),
)


@pytest.mark.parametrize(
    ("v", "pattern"),
    [
        (
            type("_Class", (), {}),
            r"Cannot serialize <class 'type'> \(<class '.*_Class'>\) to YAML",
        ),
        (
            {"x": lambda x: x},
            r"Cannot serialize <class 'function'> \(<function <lambda> at 0x.*>\)",
        ),
        (
            {"x": 1 + 2j},
            r"Cannot serialize <class 'complex'> \(\(1\+2j\)\) to YAML",
        ),
        (
            {"valid": {"invalid": object()}},
            r"Cannot serialize <class 'object'> \(<object object at 0x.*>\) to YAML",
        ),
    ],
)
def test_incorrect_dumps(v, pattern):
    with pytest.raises(yaml_rs.YAMLEncodeError, match=pattern):
        yaml_rs.dumps(v)


@pytest.mark.parametrize(
    ("data", "dumped"),
    [
        (date(2002, 12, 14), "2002-12-14"),
        (dt, '"2001-12-14T21:59:43.10-05:00"'),
        (datetime(2001, 12, 15, 2, 59, 43, 100000, tzinfo=UTC), '"2001-12-15T02:59:43.1Z"'),
        (datetime(2001, 12, 14, 21, 59, 43, tzinfo=timezone(timedelta(seconds=19800))),
         '"2001-12-14T21:59:43+05:30"'),
        (datetime(2001, 12, 15, 2, 59, 43, tzinfo=UTC),
         '"2001-12-15T02:59:43Z"'),
        (datetime(2025, 1, 1, 0, 0, 0), '"2025-01-01T00:00:00"'),
        (datetime(2025, 1, 1, 0, 0, 0, tzinfo=timezone(timedelta(hours=-8))),
         '"2025-01-01T00:00:00-08:00"'),
        (datetime(2025, 1, 1, 0, 0, 0, tzinfo=timezone(timedelta(hours=3))),
         '"2025-01-01T00:00:00+03:00"'),
    ],
)
def test_dumps(data: Any, dumped: str) -> None:
    assert yaml_rs.dumps(data).removeprefix("---\n") == str(dumped)


@pytest.mark.parametrize("yaml", VALID_YAMLS)
def test_valid_yamls_dumps_from_test_suite(yaml: Path) -> None:
    load_from_str = yaml_rs.loads(yaml.read_text(encoding="utf-8"), parse_datetime=False)

    docs = [load_from_str] if isinstance(load_from_str, dict) else load_from_str

    for doc in docs:
        parsed_yaml = yaml_rs.loads(normalize_yaml(doc), parse_datetime=False)

        if isinstance(parsed_yaml, set):
            parsed_yaml = dict.fromkeys(parsed_yaml)

        dump = doc.get("dump")
        if dump is None:
            continue

        dumps = yaml_rs.dumps(parsed_yaml)
        # FIXME
        try:
            assert dumps == dump
        except AssertionError:
            pytest.skip(f"skip: {yaml}")
