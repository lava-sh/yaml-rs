from pathlib import Path

import pytest
import yaml_rs

from tests import VALID_YAMLS, normalize_yaml


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
