from datetime import date, datetime, time, timedelta, timezone
from decimal import Decimal
from textwrap import dedent
from typing import Any

import pytest
import yaml as pyyaml
import yaml_rs

from .helpers import UTC, dt


@pytest.mark.parametrize(
    ("obj", "exc_msg"),
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
def test_incorrect_dumps(obj: Any, exc_msg: str) -> None:
    with pytest.raises(yaml_rs.YAMLEncodeError, match=exc_msg):
        yaml_rs.dumps(obj)


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
        (time(10, 30, 0, 100000), '"10:30:00.1"'),
        (time(10, 30, 0, 120000), '"10:30:00.12"'),
        (time(10, 30, 0, 123400), '"10:30:00.1234"'),
    ],
)  # fmt: off
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


# https://github.com/lava-sh/yaml-rs/issues/131
def test_dumps_escapes_unsafe_scalar_chars() -> None:
    data = {
        "nel": "line1\x85line2",
        "ls": "line1\u2028line2",
        "ps": "line1\u2029line2",
        "bom": "line1\ufeffline2",
        "controls": "\0\a\b\v\f\r\x1b\x7f\x80\x9f",
    }

    dumped = yaml_rs.dumps(data)

    for unsafe_chars in "\x85\u2028\u2029\ufeff\0\a\b\v\f\r\x1b\x7f\x80\x9f":
        assert unsafe_chars not in dumped

    assert yaml_rs.loads(dumped) == data
    assert pyyaml.safe_load(dumped) == data


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


@pytest.mark.parametrize(
    ("obj", "expected"),
    [
        (Decimal(0), "x: 0.0"),
        (Decimal(1), "x: 1.0"),
        (Decimal(-1), "x: -1.0"),
        # # #
        (Decimal("1E+3"), "x: 1e+3"),
        (Decimal("1e3"), "x: 1e+3"),
        (Decimal("-1e-3"), "x: -0.001"),
        # # #
        (Decimal("1.5"), "x: 1.5"),
        (Decimal("-0.25"), "x: -0.25"),
        # # #
        (Decimal(42), "x: 42.0"),
        (Decimal(-42), "x: -42.0"),
        # # #
        (Decimal("1.000"), "x: 1.000"),
        (Decimal("0.0001"), "x: 0.0001"),
        # # #
        (Decimal("Infinity"), "x: .inf"),
        (Decimal("+Infinity"), "x: .inf"),
        (Decimal("-Infinity"), "x: -.inf"),
        # # #
        (Decimal("NaN"), "x: .nan"),
        (Decimal("sNaN"), "x: .nan"),
        (Decimal("+NaN"), "x: .nan"),
        (Decimal("-NaN"), "x: .nan"),
        # # #
        (Decimal(" 1 "), "x: 1.0"),  # noqa: FURB157
        (Decimal("  -2.5  "), "x: -2.5"),
    ],
)
def test_dumps_decimal(obj: Decimal, expected: str) -> None:
    assert yaml_rs.dumps({"x": obj}).removeprefix("---\n") == expected


@pytest.mark.parametrize(
    "obj",
    [
        "  indented\nnext",
        "  indented\n  still indented",
        "\tindented\nnext",
    ],
)
# https://github.com/lava-sh/yaml-rs/issues/130
def test_leading_indent_round_trip(obj: str) -> None:
    data = {
        "lead": obj,
        "nested": {"lead": obj},
    }

    dumped = yaml_rs.dumps(data, multiline_strings=True)

    assert yaml_rs.loads(dumped) == data
    assert pyyaml.safe_load(dumped) == data


@pytest.mark.parametrize(
    "obj",
    [
        "\n",
        "\n\n",
        "line\n",
        "line\n\n",
    ],
)
# # https://github.com/lava-sh/yaml-rs/issues/130
def test_trailing_newline_round_trip(obj: str) -> None:
    data = {"value": obj}

    dumped = yaml_rs.dumps(data, multiline_strings=True)

    assert yaml_rs.loads(dumped) == data
    assert pyyaml.safe_load(dumped) == data
