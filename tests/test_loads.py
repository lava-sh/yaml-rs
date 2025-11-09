import json
import sys
from datetime import date, datetime, timedelta, timezone

import pytest
import yaml_rs

from tests import VALID_YAMLS

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
    ("bad_yaml", "exc_msg"),
    [
        (
            "[ [ [ [",
            "YAML parse error at line 2, column 1\n"
            "while parsing a node, did not find expected node content",
        ),
        (
            'name: "unclosed',
            """\
YAML parse error at line 1, column 7
  |
1 | name: "unclosed
  |       ^
while scanning a quoted scalar, found unexpected end of stream""",
        ),
    ],
)
def test_loads_errors(bad_yaml: str, exc_msg: str) -> None:
    with pytest.raises(yaml_rs.YAMLDecodeError) as exc_info:
        yaml_rs.loads(bad_yaml)
    assert str(exc_info.value) == exc_msg


@pytest.mark.parametrize(
    ("yaml", "parsed"),
    [
        (
            "canonical: 2001-12-15T02:59:43.1Z",
            {"canonical": datetime(2001, 12, 15, 2, 59, 43, 100000, tzinfo=UTC)},
        ),
        (
            "date: 2002-12-14",
            {"date": date(2002, 12, 14)},
        ),
        ("spaced: 2001-12-14 21:59:43.10 -5", {"spaced": dt}),
        ("iso8601: 2001-12-14t21:59:43.10-05:00", {"iso8601": dt}),
        ("not-date: !!str 2002-04-28", {"not-date": "2002-04-28"}),
    ],
)
def test_parse_datetime(yaml: str, parsed) -> None:
    assert yaml_rs.loads(yaml) == parsed


@pytest.mark.parametrize("yaml", VALID_YAMLS)
def test_yaml_test_suite(yaml):
    load_from_str = yaml_rs.loads(yaml.read_text(encoding="utf-8"), parse_datetime=False)
    if isinstance(load_from_str, dict):
        docs = [load_from_str]
    elif isinstance(load_from_str, list):
        docs = load_from_str
    else:
        pytest.skip("")
    d = docs[0]
    normalize_yaml = (
        d.get("yaml")
        # https://github.com/yaml/yaml-test-suite#special-characters
        # https://github.com/saphyr-rs/saphyr/blob/v0.0.6/parser/tests/yaml-test-suite.rs#L312-L318
        .replace("␣", " ")
        .replace("»", "\t")
        .replace("—", "")  # Tab line continuation ——»
        .replace("←", "\r")
        .replace("⇔", "\ufeff")  # BOM character
        .replace("↵", "")  # Trailing newline marker
        .replace("∎\n", "")
    )
    parsed_yaml = yaml_rs.loads(normalize_yaml, parse_datetime=False)
    # FIXME
    try:
        parsed_json = json.loads(d.get("json"))
    except json.decoder.JSONDecodeError:
        pytest.skip(f"Skipping {yaml.name}")
    assert parsed_yaml == parsed_json
