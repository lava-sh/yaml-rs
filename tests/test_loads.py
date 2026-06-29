import math
import platform
from datetime import date, datetime, timedelta, timezone
from typing import Any, Literal

import pytest
import yaml_rs
from yaml_rs import AliasLimits, DuplicateKeyPolicy, YAMLDecodeError

from .helpers import UTC, dt, is_nan


@pytest.mark.parametrize(
    ("yaml", "exc_msg"),
    [
        (
            "[ [ [ [",
            """\
YAML parse error at line 1, column 7
  |
1 | [ [ [ [
  |       ^
unclosed bracket '['""",
        ),
        (
            'name: "unclosed',
            """\
YAML parse error at line 1, column 7
  |
1 | name: "unclosed
  |       ^
unclosed quote""",
        ),
        (
            "*",
            """\
YAML parse error at line 1, column 1
  |
1 | *
  | ^
while scanning an anchor or alias, did not find expected alphabetic or numeric character""",
        ),
        # Test case 4H7K: extra closing bracket is an error
        (
            "[ a, b, c ] ]",
            """\
YAML parse error at line 1, column 13
  |
1 | [ a, b, c ] ]
  |             ^
misplaced bracket""",
        ),
        # Test case BS4K: comment intercepts multiline content
        (
                """\
word1  # comment
word2
                """,
                """\
YAML parse error at line 1, column 8
  |
1 | word1  # comment
  |        ^
comment intercepting the multiline text""",
        ),
        ("x: !!bool 1", "Invalid value '1' for '!!bool' tag"),
        ("x: !!bool 3.14", "Invalid value '3.14' for '!!bool' tag"),
        # ______________________________________________________
        ("x: !!int true", "Invalid value 'true' for '!!int' tag"),
        # _________________________________________
        ("x: !!invalid", "Invalid tag: '!!invalid'"),
    ],
)  # fmt: off
def test_yaml_loads_decode_error(yaml: str, exc_msg: str) -> None:
    with pytest.raises(yaml_rs.YAMLDecodeError) as exc_info:
        yaml_rs.loads(yaml)

    assert str(exc_info.value) == exc_msg


@pytest.mark.parametrize(
    ("bad", "exc_msg"),
    [
        (5, "Expected str object, not 'int'"),
        ({1, 2}, "Expected str object, not 'set'"),
        ([1, 2], "Expected str object, not 'list'"),
    ],
)
def test_yaml_loads_type_error(bad: str, exc_msg: str) -> None:
    with pytest.raises(TypeError) as exc_info:
        yaml_rs.loads(bad)
    assert str(exc_info.value) == exc_msg


@pytest.mark.parametrize(
    ("data", "encoding", "encoder_errors", "expected_error"),
    [
        (
            b"\xff\xfe",
            "utf-8",
            "strict",
            "failed to encode bytes: invalid utf-8 sequence",
        ),
        (b"test", "utf-8", "qsfasf", "invalid decoder: qsfasf"),
        (b"test", "asdfas", None, "invalid encoding: asdfas"),
        (b"\x81", "shift_jis", "strict", "decoding error: malformed input"),
        (b"\xff", "iso-2022-jp", "strict", "decoding error: malformed input"),
        (
            b"test",
            "windows-1252",
            "unknown_handler",
            "invalid decoder: unknown_handler",
        ),
        (b"\x81", "shift-jis", "strict", "decoding error: malformed input"),
        (b"\x81", "sjis", "strict", "decoding error: malformed input"),
        (b"\x81", "big5", "strict", "decoding error: malformed input"),
        (b"\x81", "gbk", "strict", "decoding error: malformed input"),
        (b"\x81", "gb18030", "strict", "decoding error: malformed input"),
        (b"\x81", "euc-kr", "strict", "decoding error: malformed input"),
        (b"\x81", "euckr", "strict", "decoding error: malformed input"),
        (b"\x81", "euc-jp", "strict", "decoding error: malformed input"),
        (b"\x81", "eucjp", "strict", "decoding error: malformed input"),
    ],
)
def test_yaml_load_encoding_errors(
    data: Any,
    encoding: str,
    encoder_errors: Literal["ignore", "replace", "strict"] | None,
    expected_error: str,
) -> None:
    with pytest.raises(yaml_rs.YAMLDecodeError) as exc_info:
        yaml_rs.load(data, encoding=encoding, encoder_errors=encoder_errors)
    assert expected_error == str(exc_info.value)


@pytest.mark.parametrize(
    ("data", "encoding", "expected"),
    [
        (b"test", "utf-8", "test"),
        (b"test", "shift_jis", "test"),
        (b"test", "shift-jis", "test"),
        (b"test", "sjis", "test"),
        (b"test", "big5", "test"),
        (b"test", "gbk", "test"),
        (b"test", "gb18030", "test"),
        (b"test", "euc-kr", "test"),
        (b"test", "euckr", "test"),
        (b"test", "iso-2022-jp", "test"),
        (b"test", "windows-1252", "test"),
        (b"test", "cp1252", "test"),
        (b"test", "windows-1251", "test"),
        (b"test", "windows-1250", "test"),
        (b"test", "iso-8859-1", "test"),
        (b"test", "latin1", "test"),
        (b"test", "iso-8859-2", "test"),
        (b"test", "iso-8859-5", "test"),
        (b"test", "iso-8859-6", "test"),
        (b"test", "iso-8859-7", "test"),
        (b"test", "iso-8859-8", "test"),
        (b"test", "euc-jp", "test"),
        (b"test", "eucjp", "test"),

        (b"\x82\xa0", "shift_jis", "あ"),
        (b"\xa4\x40", "big5", "一"),
        (b"\xb0\xa1", "euc-kr", "가"),
        (b"\x81\x40", "gbk", "丂"),

        (b"\xe4", "windows-1252", "ä"),
        (b"\xe4", "iso-8859-1", "ä"),
        (b"\xe4", "latin1", "ä"),
        (b"\xca", "windows-1251", "К"),
        (b"\xe0", "windows-1250", "ŕ"),
        (b"\xc1", "iso-8859-2", "Á"),
        (b"\xb1", "iso-8859-5", "Б"),
        (b"\xc1", "iso-8859-6", "ء"),
        (b"\xc1", "iso-8859-7", "Α"),
        (b"\xf1", "iso-8859-8", "ס"),
    ],
)  # fmt: off
def test_yaml_load_encoding_success(
    data: bytes,
    encoding: str,
    expected: str,
) -> None:
    result = yaml_rs.load(data, encoding=encoding)
    assert expected in str(result)


@pytest.mark.parametrize(
    ("yaml", "parsed"),
    [
        ("2002-12-14", date(2002, 12, 14)),
        ("2001-12-14 21:59:43.10 -5", dt),
        ("2001-12-14 21:59:43.10 -05", dt),
        ("2001-12-14 21:59:43.10  -05", dt),
        ("2001-12-14 21:59:43.10   -05", dt),
        ("2001-12-14 21:59:43.10    -05", dt),
        ("2001-12-14 21:59:43.10     -05", dt),
        ("2001-12-14 21:59:43.10                        -05", dt),
        ("2001-12-14\t21:59:43.10\t-05", dt),
        ("2001-12-14  21:59:43.10 -05", "2001-12-14  21:59:43.10 -05"),
        ("2001-13-14", "2001-13-14"),
        ("2001-02-29", "2001-02-29"),
        ("2001-12-14t21:59:43.10-05:00", dt),
        ("2001-12-14t21:59:43.10-05", dt),
        ("2001-12-15T02:59:43.1Z", datetime(2001, 12, 15, 2, 59, 43, 100000, tzinfo=UTC)),
        ("2001-12-15T02:59:43. 1   Z", "2001-12-15T02:59:43. 1   Z"),
        ("2001-12-15T25:59:43Z", "2001-12-15T25:59:43Z"),
        ("2001-12-15T02:99:43Z", "2001-12-15T02:99:43Z"),
        ("2001-12-15T02:59:60Z", "2001-12-15T02:59:60Z"),
        ("2001-12-15T02:59:43+00:99", "2001-12-15T02:59:43+00:99"),
        (
            "2001-12-14T21:59:43+05:30",
            datetime(2001, 12, 14, 21, 59, 43, tzinfo=timezone(timedelta(seconds=19800))),
        ),
        ("2001-12-15T02:59", "2001-12-15T02:59"),
        ("!!str 2002-04-28", "2002-04-28"),
        # https://github.com/yaml/yaml-spec/blob/1b1a1be4/spec/1.2/docbook/timestamp.dbk#L139
        # ([Tt]|[ \t]+)[0-9][0-9]? <lineannotation># (hour)</lineannotation>
        # `T` and `t` are allowed
        ("2001-12-15t02:59:43Z", datetime(2001, 12, 15, 2, 59, 43, tzinfo=UTC)),
        ("2001-12-15T02:59:43Z", datetime(2001, 12, 15, 2, 59, 43, tzinfo=UTC)),
        ("2001-12-15T02:59:43-0530", "2001-12-15T02:59:43-0530"),
        ("2001-12-15T02:59:43+123456", "2001-12-15T02:59:43+123456"),
        # https://github.com/yaml/yaml-spec/blob/1b1a1be4/spec/1.2/docbook/timestamp.dbk#L143
        # ([ \t]*(Z|[-+][0-9][0-9]?(:[0-9][0-9])?))? <lineannotation># (time zone)</lineannotation>
        # only `Z` allowed
        ("2001-12-15T02:59:43z", "2001-12-15T02:59:43z"),
    ],
)
def test_parse_datetime(yaml: str, parsed: Any) -> None:
    assert yaml_rs.loads(yaml, parse_datetime=True) == parsed


@pytest.mark.parametrize(
    ("yaml", "parsed"),
    [
        ("", None),
        # Example 10.1 !!map Examples
        # https://yaml.org/spec/1.2.2/#:~:text=Example%2010%2E1%20%21%21map%20Examples
        ("Block style: !!map\n"
         "  Clark : Evans\n"
         "  Ingy  : döt Net\n"
         "  Oren  : Ben-Kiki\n"
         "\n"
         "Flow style: !!map { Clark: Evans, Ingy: döt Net, Oren: Ben-Kiki }\n",
         {"Block style": {"Clark": "Evans", "Ingy": "döt Net", "Oren": "Ben-Kiki"},
          "Flow style": {"Clark": "Evans", "Ingy": "döt Net", "Oren": "Ben-Kiki"}}),
        # Example 10.2 !!seq Examples
        # https://yaml.org/spec/1.2.2/#:~:text=Example%2010%2E2%20%21%21seq%20Examples
        ("Block style: !!seq\n"
         "- Clark Evans\n"
         "- Ingy döt Net\n"
         "- Oren Ben-Kiki\n"
         "\n"
         "Flow style: !!seq [ Clark Evans, Ingy döt Net, Oren Ben-Kiki ]\n",
         {"Block style": ["Clark Evans", "Ingy döt Net", "Oren Ben-Kiki"],
          "Flow style": ["Clark Evans", "Ingy döt Net", "Oren Ben-Kiki"]}),
        # Example 10.3 !!str Examples
        # https://yaml.org/spec/1.2.2/#:~:text=Example%2010%2E3%20%21%21str%20Examples
        ("Block style: !!str |-\n"
         "  String: just a theory.\n"
         "\n"
         'Flow style: !!str "String: just a theory."\n',
         {"Block style": "String: just a theory.",
          "Flow style": "String: just a theory."}),
        # Example 10.4 !!null Examples
        # https://yaml.org/spec/1.2.2/#:~:text=Example%2010%2E4%20%21%21null%20Examples
        ("!!null null: value for null key\n"
         "key with null value: !!null null",
         {None: "value for null key", "key with null value": None}),
        # Example 10.5 !!bool Examples
        # https://yaml.org/spec/1.2.2/#:~:text=Example%2010%2E5%20%21%21bool%20Examples
        ("YAML is a superset of JSON: !!bool true\n"
         "Pluto is a planet: !!bool false",
         {"Pluto is a planet": False, "YAML is a superset of JSON": True}),
        # Example 10.6 !!int Examples
        # https://yaml.org/spec/1.2.2/#:~:text=Example%2010%2E6%20%21%21int%20Examples
        ("negative: !!int -12\n"
         "zero: !!int 0\n"
         "positive: !!int 34\n",
         {"negative": -12, "positive": 34, "zero": 0}),
        # Example 10.7 !!float Examples
        # https://yaml.org/spec/1.2.2/#:~:text=Example%2010%2E7%20%21%21float%20Examples
        ("negative: !!float -1\n"
         "zero: !!float 0\n"
         "positive: !!float 2.3e4\n"
         "infinity: !!float .inf\n"
         "not a number: !!float .nan\n",
         {"infinity": float("inf"),
          "negative": -1.0,
          "not a number": float("nan"),
          "positive": 23000.0,
          "zero": 0.0}),
        # Example 10.8 JSON Tag Resolution
        # https://yaml.org/spec/1.2.2/#:~:text=Example%2010%2E8%20JSON%20Tag%20Resolution
        ("A null: null\n"
         "Booleans: [ true, false ]\n"
         "Integers: [ 0, -0, 3, -19 ]\n"
         "Floats: [ 0., -0.0, 12e03, -2E+05 ]\n"
         "Invalid: [ True, Null,\n"
         "  0o7, 0x3A, +12.3 ]\n",
         {"A null": None,
          "Booleans": [True, False],
          "Floats": [0.0, -0.0, 12000.0, -200000.0],
          "Integers": [0, 0, 3, -19],
          "Invalid": [True, None, 7, 58, 12.3]}),
        # Example 10.9 Core Tag Resolution
        # https://yaml.org/spec/1.2.2/#:~:text=Example%2010%2E9%20Core%20Tag%20Resolution
        ("A null: null\n"
         "Also a null: # Empty\n"
         'Not a null: ""\n'
         "Booleans: [ true, True, false, FALSE ]\n"
         "Integers: [ 0, 0o7, 0x3A, -19 ]\n"
         "Floats: [\n"
         "  0., -0.0, .5, +12e03, -2E+05 ]\n"
         "Also floats: [\n"
         "  .inf, -.Inf, +.INF, .NAN ]\n",
         {"A null": None,
          "Also a null": None,
          "Also floats": [float("inf"), float("-inf"), float("inf"), float("nan")],
          "Booleans": [True, True, False, False],
          "Floats": [0.0, -0.0, 0.5, 12000.0, -200000.0],
          "Integers": [0, 7, 58, -19],
          "Not a null": ""}),
        # # # # # # # # # # #
        ("all_bools: [ true , True , TRUE , false , False , FALSE ]",
         {"all_bools": [True, True, True, False, False, False]}),
        ("all_nulls: [ null , Null , NULL , ~ ]",
         {"all_nulls": [None, None, None, None]}),
        ("~: null", {None: None}),
        ("null: ~", {None: None}),
        ("null: null", {None: None}),
        ("~: ~", {None: None}),
        ("NULL: ~", {None: None}),
        ("~: ~\n"
         "--- !!set\n"
         "? Mark McGwire\n"
         "? Sammy Sosa\n"
         "? Ken Griffey",
         [{None: None}, {"Mark McGwire", "Sammy Sosa", "Ken Griffey"}]),
        # https://github.com/saphyr-rs/saphyr/issues/84
        (
            """
            hello:
              world: this is a string
                --- still a string
            """,
            {"hello": {"world": "this is a string --- still a string"}},
        ),
    ],
)  # fmt: off
def test_parse_yaml_spec_examples(yaml: str, parsed: Any) -> None:
    assert yaml_rs.loads(yaml) == is_nan(parsed), (
        f"\nRaw: {yaml}\n"
        f"\nParsed: {parsed}\n"
    )  # fmt: off


@pytest.mark.parametrize(
    ("yaml", "parsed"),
    [
        # This valid, because Core Schema tags on collections are ignored,
        # since the syntax disallows any ambiguity in parsing.
        ("x: !!bool [ 1, 2, 3 ]", {"x": [1, 2, 3]}),
        # Also valid cases
        ("x: !!null ~", {"x": None}),
        ("x: !!null Null", {"x": None}),
        ("x: !!null NULL", {"x": None}),
        ("x: !!null null", {"x": None}),
    ],
)
def test_parse_yaml_tags(yaml: str, parsed: Any) -> None:
    assert yaml_rs.loads(yaml) == parsed


@pytest.mark.skipif(
    platform.python_implementation() == "PyPy",
    reason="PyPy's `Decimal` parsing hits the int string "
           "conversion digit limit for very large numbers.",
)  # fmt: off
def test_parse_big_nums() -> None:
    big_int = 999**999
    big_float = float(f"{big_int}.{big_int}")

    y = f"x: {big_int}"
    y2 = f"x: .{big_float}"  # x: .inf
    y3 = f"x: {big_int + big_int}.{big_int}"
    y4 = f"x: -{big_int}"

    assert yaml_rs.loads(y)["x"] == big_int
    assert yaml_rs.loads(y4)["x"] == -big_int
    assert math.isclose(
        yaml_rs.loads(y2)["x"],
        float("inf"),
        abs_tol=1e-9,
    )
    assert math.isclose(
        yaml_rs.loads(y3)["x"],
        big_float,
        abs_tol=1e-9,
    )


@pytest.mark.parametrize(
    ("kwargs", "exc_type", "exc_msg"),
    [
        (
            {"max_total_replayed_events": -1},
            ValueError,
            "`max_total_replayed_events` must be greater than or equal to 0",
        ),
        (
            {"max_replay_stack_depth": -1},
            ValueError,
            "`max_replay_stack_depth` must be greater than or equal to 0",
        ),
        (
            {"max_alias_expansions_per_anchor": -1},
            ValueError,
            "`max_alias_expansions_per_anchor` must be greater than or equal to 0",
        ),
    ],
)
# https://github.com/lava-sh/yaml-rs/issues/124
def test_alias_limits_invalid_constructor_args(
    kwargs: dict[str, Any],
    exc_type: type[Exception],
    exc_msg: str,
) -> None:
    with pytest.raises(exc_type, match=exc_msg):
        AliasLimits(**kwargs)


@pytest.mark.parametrize(
    ("yaml", "limits", "exc_msg"),
    [
        pytest.param(
            "defs: &A { k: v }\nx: *A\ny: *A\nz: *A\n",
            AliasLimits(max_alias_expansions_per_anchor=2),
            "alias expansion limit exceeded",
            id="expansion_limit",
        ),
        pytest.param(
            "defs: &A [1, 2, 3, 4]\nlist: [*A, *A]\n",
            AliasLimits(max_total_replayed_events=10),
            "alias replay limit exceeded",
            id="total_replay_limit",
        ),
        pytest.param(
            "defs: &A [1]\nout: *A\n",
            AliasLimits(max_replay_stack_depth=0),
            "alias replay stack depth exceeded",
            id="stack_depth_limit",
        ),
        pytest.param(
            """\
            a: &a ~
            b: &b [*a,*a,*a,*a,*a,*a,*a,*a,*a]
            c: &c [*b,*b,*b,*b,*b,*b,*b,*b,*b]
            d: &d [*c,*c,*c,*c,*c,*c,*c,*c,*c]
            e: &e [*d,*d,*d,*d,*d,*d,*d,*d,*d]
            f: &f [*e,*e,*e,*e,*e,*e,*e,*e,*e]
            g: &g [*f,*f,*f,*f,*f,*f,*f,*f,*f]
            h: &h [*g,*g,*g,*g,*g,*g,*g,*g,*g]
            i: &i [*h,*h,*h,*h,*h,*h,*h,*h,*h]
            j: &j [*i,*i,*i,*i,*i,*i,*i,*i,*i]
            k: &k [*j,*j,*j,*j,*j,*j,*j,*j,*j]
            l: &l [*k,*k,*k,*k,*k,*k,*k,*k,*k]
            m: &m [*l,*l,*l,*l,*l,*l,*l,*l,*l]
            n: &n [*m,*m,*m,*m,*m,*m,*m,*m,*m]
            o: &o [*n,*n,*n,*n,*n,*n,*n,*n,*n]
            p: &p [*o,*o,*o,*o,*o,*o,*o,*o,*o]
            q: &q [*p,*p,*p,*p,*p,*p,*p,*p,*p]
            r: &r [*q,*q,*q,*q,*q,*q,*q,*q,*q]
            s: &s [*r,*r,*r,*r,*r,*r,*r,*r,*r]
            t: &t [*s,*s,*s,*s,*s,*s,*s,*s,*s]
            u: &u [*t,*t,*t,*t,*t,*t,*t,*t,*t]
            v: &v [*u,*u,*u,*u,*u,*u,*u,*u,*u]
            w: &w [*v,*v,*v,*v,*v,*v,*v,*v,*v]
            x: &x [*w,*w,*w,*w,*w,*w,*w,*w,*w]
            y: &y [*x,*x,*x,*x,*x,*x,*x,*x,*x]
            z: &z [*y,*y,*y,*y,*y,*y,*y,*y,*y]
            """,
            None,
            "alias replay limit exceeded: replayed 1000006, max 1000000",
            id="alias_replay_limit_exceeded",
        ),
    ],
)
# https://github.com/lava-sh/yaml-rs/issues/124
def test_alias_limits(
    yaml: str,
    limits: AliasLimits,
    exc_msg: str,
) -> None:
    with pytest.raises(YAMLDecodeError, match=exc_msg):
        yaml_rs.loads(yaml, alias_limits=limits)


# https://github.com/lava-sh/yaml-rs/issues/128
def test_mapping_with_null_values_is_dict() -> None:
    yaml = """\
    test:
        key_a: null
        key_b: null

    test2:
        key_a: null
        key_b: "b"
    """

    assert yaml_rs.loads(yaml) == {
        "test": {"key_a": None, "key_b": None},
        "test2": {"key_a": None, "key_b": "b"},
    }


@pytest.mark.parametrize(
    ("yaml", "expected"),
    [
        ("x: 1.2x\n", "1.2x"),
        ("x: 1.2.3\n", "1.2.3"),
        ("x: .e1\n", ".e1"),
    ],
)
def test_invalid_float_like_scalars(yaml: str, expected: str) -> None:
    assert yaml_rs.loads(yaml) == {"x": expected}


@pytest.mark.parametrize(
    ("loader", "kwargs", "yaml", "expected", "exc_msg"),
    [
        pytest.param(
            yaml_rs.loads,
            {},
            "x: 1\nx: 2\nx: 3\n",
            None,
            r"duplicate mapping key: 'x'",
            id="loads_default_error",
        ),
        pytest.param(
            yaml_rs.loads,
            {"duplicate_key_policy": DuplicateKeyPolicy.LastWins},
            "x: 1\nx: 2\nx: 3\n",
            {"x": 3},
            None,
            id="loads_last_wins",
        ),
        pytest.param(
            yaml_rs.load,
            {"duplicate_key_policy": DuplicateKeyPolicy.LastWins, "encoding": "utf-8"},
            "x: 1\nx: 2\nx: 3\n",
            {"x": 3},
            None,
            id="load_last_wins",
        ),
        pytest.param(
            yaml_rs.loads,
            {"duplicate_key_policy": DuplicateKeyPolicy.FirstWins},
            "x: 1\nx: 2\nx: 3\n",
            {"x": 1},
            None,
            id="first_wins",
        ),
        pytest.param(
            yaml_rs.loads,
            {"duplicate_key_policy": DuplicateKeyPolicy.Error},
            "x: 1\nx: 2\nx: 3\n",
            None,
            r"duplicate mapping key: 'x'",
            id="error_string_key",
        ),
        pytest.param(
            yaml_rs.loads,
            {"duplicate_key_policy": DuplicateKeyPolicy.Error},
            "1: a\n1: b\n",
            None,
            r"duplicate mapping key: 1",
            id="error_non_string_key",
        ),
    ],
)
def test_duplicate_key_policy(
    loader: Any,
    kwargs: dict[str, Any],
    yaml: str,
    expected: dict[str, Any] | None,
    exc_msg: str | None,
) -> None:
    source: str | bytes = yaml.encode() if loader is yaml_rs.load else yaml

    if exc_msg is not None:
        with pytest.raises(YAMLDecodeError, match=exc_msg):
            loader(source, **kwargs)
    else:
        assert loader(source, **kwargs) == expected


@pytest.mark.parametrize(
    ("yamls", "expected"),
    [
        ([".nan", ".NaN", ".NAN"], float("nan")),
        (["nan", "NaN", "NAN"], None),
        (["+.nan", "-.nan", "+.NaN", "-.NaN"], None),
        ([".inf", ".Inf", ".INF", "+.inf", "+.Inf", "+.INF"], float("inf")),
        (["-.inf", "-.Inf", "-.INF"], float("-inf")),
        (["inf", "Inf", "INF", "+inf", "-inf"], None),
    ],
)
# https://github.com/lava-sh/yaml-rs/issues/134
def test_nan_inf(yamls: list[str], expected: Any) -> None:
    for yaml in yamls:
        result = yaml_rs.loads(yaml)

        if expected is None:
            assert result == yaml
        elif math.isnan(expected):
            assert isinstance(result, float)
            assert math.isnan(result)
        else:
            assert result == expected
