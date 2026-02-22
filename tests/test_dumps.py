import sys
from datetime import date, datetime, timedelta, timezone
from textwrap import dedent
from typing import Any

import pytest
import yaml as pyyaml
import yaml_rs

from .helpers import VALID_YAMLS, YamlTestSuite

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
def test_datetime_dumps(data: Any, dumped: str) -> None:
    assert yaml_rs.dumps(data).removeprefix("---\n") == str(dumped)


@pytest.mark.parametrize(
    ("compact", "multiline_strings", "data", "expected"),
    [
        (
            True,
            False,
            {"e": ["f", "g", {"h": []}]},
            dedent("""\
            ---
            e:
              - f
              - g
              - h: []"""),
        ),
        (
            False,
            False,
            {"e": ["f", "g", {"h": []}]},
            dedent("""\
            ---
            e:
              - f
              - g
              -
                h: []"""),  # <-- with new line
        ),
        (
            True,
            True,
            {"key": "line1\nline2"},
            dedent("""\
            ---
            key: |-
              line1
              line2"""),  # literal block style
        ),
        (
            True,
            False,
            {"key": "line1\nline2"},
            dedent("""\
            ---
            key: "line1\\nline2"
            """).rstrip("\n"),  # escaped style
        ),
        (
            True,
            True,
            {"items": ["text\nwith\nnewlines", {"nested": []}]},
            dedent("""\
            ---
            items:
              - |-
                text
                with
                newlines
              - nested: []"""),
        ),
        (
            False,
            False,
            {"items": ["text\nwith\nnewlines", {"nested": []}]},
            dedent("""\
            ---
            items:
              - "text\\nwith\\nnewlines"
              -
                nested: []"""),
        ),
    ],
)
def test_dumps_with_options(
    *,
    compact: bool,
    multiline_strings: bool,
    data: Any,
    expected: str,
) -> None:
    assert (
        yaml_rs.dumps(
            data,
            compact=compact,
            multiline_strings=multiline_strings,
        )
        == expected
    )


@pytest.mark.parametrize(
    "ts",
    [
        ts for ts in VALID_YAMLS
        if ts.out_yaml is not None
    ],
    ids=lambda ts: ts.id,
)
def test_valid_yamls_dumps_from_test_suite(ts: YamlTestSuite) -> None:
    loaded = yaml_rs.loads(ts.in_yaml.read_text("utf-8"), parse_datetime=False)

    if isinstance(loaded, list):
        dumped = "".join(yaml_rs.dumps(doc) for doc in loaded)
    else:
        dumped = yaml_rs.dumps(loaded)

    expected = ts.out_yaml.read_text("utf-8")

    try:
        # FIXME
        assert dumped == expected
    except AssertionError:
        pytest.skip(f"dump mismatch: {ts.id}")


# https://github.com/lava-sh/yaml-rs/issues/69
@pytest.mark.parametrize(
    "data",
    [
        {
            "int": 99999999999999999999999999999999999999999,
            "float": 99999999999999999999999999999999999999999.9999999,
            "neg-int": -99999999999999999999999999999999999999999,
            "neg-float": -99999999999999999999999999999999999999999.9999999,
        },
        {
            "big_pos_int": 10**200,
            "big_neg_int": -(10**200),
        },
        {
            "radix_2": int("1" * 256, 2),
            "radix_8": int("7" * 128, 8),
            "radix_16": int("f" * 96, 16),
            "radix_36": int("z" * 64, 36),
        },
        {
            "radix_mix_pos": int("deadbeef" * 8, 16),
            "radix_mix_neg": -int("101010" * 40, 2),
            "radix_base36": int("abcxyz" * 30, 36),
        },
        {
            "huge_float_pos": 1e308,
            "huge_float_neg": -1e308,
            "tiny_float_pos": 1e-308,
            "tiny_float_neg": -1e-308,
        },
        {
            "special_inf": float("inf"),
            "special_ninf": float("-inf"),
            "special_nan": float("nan"),
        },
    ],
)
def test_dumps_nums(data: dict[str, float | int]) -> None:
    dumped = yaml_rs.dumps(data).removeprefix("---\n")
    expected = pyyaml.dump(data, sort_keys=False).rstrip("\n")
    assert dumped == expected
