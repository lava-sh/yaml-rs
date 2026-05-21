import json

import pytest
import yaml_rs
from yaml_rs import DuplicateKeyPolicy

from .helpers import is_nan
from .yaml_test_suite import INVALID_YAMLS, SKIPPED_YAMLS, VALID_YAMLS, YamlTestSuite


@pytest.mark.parametrize("ts", VALID_YAMLS, ids=lambda ts: ts.id)
def test_valid_yamls_from_test_suite(ts: YamlTestSuite) -> None:
    actual = yaml_rs.loads(
        ts.in_yaml.read_text("utf-8"),
        parse_datetime=False,
    )

    text = ts.in_json.read_text("utf-8")

    if text == "":  # noqa: PLC1901
        expected = None
    else:
        try:
            expected = json.loads(text)
        except json.JSONDecodeError:
            decoder = json.JSONDecoder()
            expected = []
            pos = 0
            n = len(text)

            while pos < n:
                obj, pos = decoder.raw_decode(text, pos)
                expected.append(obj)
                while pos < n and text[pos] in " \t\r\n":
                    pos += 1

    if isinstance(expected, list) and not isinstance(actual, list):
        actual = [actual]

    # JSON does not have a native "set" type, while Python does.
    # In YAML, the tag `!!set` represents a set, and Python YAML parsers
    # (including ours) map it to a Python `set`.
    # ```python
    # import yaml as py_yaml
    #
    # y = """\
    # --- !!set
    # ? Mark McGwire
    # ? Sammy Sosa
    # ? Ken Griffey
    # """
    # print(py_yaml.safe_load(y))  # {'Mark McGwire', 'Ken Griffey', 'Sammy Sosa'}
    # print(type(py_yaml.safe_load(y)))  # <class 'set'>
    # ```
    if (
        isinstance(actual, set)
        and isinstance(expected, dict)
        and all(v is None for v in expected.values())
    ):
        actual = dict.fromkeys(actual)

    assert is_nan(actual) == is_nan(expected), (
        f"\nTest case: {ts.id}\n"
        f"\nYAML file: {ts.in_yaml}\n"
        f"\nActual:\n{actual!r}\n"
        f"\nExpected:\n{expected!r}\n"
    )


@pytest.mark.parametrize("ts", SKIPPED_YAMLS, ids=lambda ts: ts.id)
def test_skipped_yamls_from_test_suite(ts: YamlTestSuite) -> None:
    # For these cases, there are no `.json` files in `yaml-test-suite`,
    # so we have nothing to compare them to, just check that they load without error.
    yaml_rs.loads(
        ts.in_yaml.read_text("utf-8"),
        parse_datetime=False,
        duplicate_key_policy=DuplicateKeyPolicy.LastWins,
    )


@pytest.mark.parametrize("ts", INVALID_YAMLS, ids=lambda ts: ts.id)
def test_invalid_yamls_from_test_suite(ts: YamlTestSuite) -> None:
    with pytest.raises(yaml_rs.YAMLDecodeError):
        yaml_rs.loads(ts.in_yaml.read_text("utf-8"), parse_datetime=False)
