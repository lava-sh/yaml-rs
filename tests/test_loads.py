# ruff: noqa: E501
import json
import sys
from datetime import date, datetime, timedelta, timezone
from pathlib import Path
from typing import Any

import pytest
import yaml_rs

from tests import INVALID_YAMLS, VALID_YAMLS, _is_nan, normalize_yaml

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
        (
            "*",
            """\
YAML parse error at line 1, column 1
  |
1 | *
  | ^
while scanning an anchor or alias, did not find expected alphabetic or numeric character""",
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
        ("2002-12-14", date(2002, 12, 14)),
        ("2001-12-14 21:59:43.10 -5", dt),
        ("2001-12-14 21:59:43.10 -05", dt),
        ("2001-12-14 21:59:43.10  -05", dt),
        ("2001-12-14 21:59:43.10   -05", dt),
        ("2001-12-14 21:59:43.10    -05", dt),
        ("2001-12-14 21:59:43.10     -05", dt),
        ("2001-12-14 21:59:43.10                        -05", dt),
        ("2001-12-14t21:59:43.10-05:00", dt),
        ("2001-12-14t21:59:43.10-05", dt),
        ("2001-12-15T02:59:43.1Z", datetime(2001, 12, 15, 2, 59, 43, 100000, tzinfo=UTC)),
        ("2001-12-15T02:59:43. 1   Z", "2001-12-15T02:59:43. 1   Z"),
        (
            "2001-12-14T21:59:43+05:30",
            datetime(2001, 12, 14, 21, 59, 43, tzinfo=timezone(timedelta(seconds=19800))),
        ),
        ("!!str 2002-04-28", "2002-04-28"),
        # https://github.com/yaml/yaml-spec/blob/1b1a1be4/spec/1.2/docbook/timestamp.dbk#L139
        # ([Tt]|[ \t]+)[0-9][0-9]? <lineannotation># (hour)</lineannotation>
        # `T` and `t` are allowed
        ("2001-12-15t02:59:43Z", datetime(2001, 12, 15, 2, 59, 43, tzinfo=UTC)),
        ("2001-12-15T02:59:43Z", datetime(2001, 12, 15, 2, 59, 43, tzinfo=UTC)),
        # https://github.com/yaml/yaml-spec/blob/1b1a1be4/spec/1.2/docbook/timestamp.dbk#L143
        # ([ \t]*(Z|[-+][0-9][0-9]?(:[0-9][0-9])?))? <lineannotation># (time zone)</lineannotation>
        # only `Z` allowed
        ("2001-12-15T02:59:43z", "2001-12-15T02:59:43z"),
    ],
)
def test_parse_datetime(yaml: str, parsed: Any) -> None:
    assert yaml_rs.loads(yaml, parse_datetime=True) == parsed


# fmt: off
@pytest.mark.parametrize(
    ("yaml", "parsed"),
    [
        ("", None),
        # Example 2.1 Sequence of Scalars (ball players)
        ("- Mark McGwire\n"
         "- Sammy Sosa\n"
         "- Ken Griffey",
         ["Mark McGwire", "Sammy Sosa", "Ken Griffey"]),
        # Example 2.3 Mapping Scalars to Sequences (ball clubs in each league)
        ("american:\n- "
         "Boston Red Sox\n"
         "- Detroit Tigers\n"
         "- New York Yankees\n"
         "national:\n"
         "- New York Mets\n"
         "- Chicago Cubs\n"
         "- Atlanta Braves\n",
         {"american": ["Boston Red Sox", "Detroit Tigers", "New York Yankees"],
          "national": ["New York Mets", "Chicago Cubs", "Atlanta Braves"]}),
        # Example 2.4 Sequence of Mappings (players’ statistics)
        ("-\n"
         "  name: Mark McGwire\n"
         "  hr:   65\n"
         "  avg:  0.278\n"
         "-\n"
         "  name: Sammy Sosa\n"
         "  hr:   63\n"
         "  avg:  0.288\n",
         [{"avg": 0.278, "hr": 65, "name": "Mark McGwire"},
          {"avg": 0.288, "hr": 63, "name": "Sammy Sosa"}]),
        # Example 2.5 Sequence of Sequences
        ("- [name        , hr, avg  ]\n"
         "- [Mark McGwire, 65, 0.278]\n"
         "- [Sammy Sosa  , 63, 0.288]\n",
         [["name", "hr", "avg"], ["Mark McGwire", 65, 0.278], ["Sammy Sosa", 63, 0.288]]),
        # Example 2.6 Mapping of Mappings
        ("Mark McGwire: {hr: 65, avg: 0.278}\n"
         "Sammy Sosa: {\n"
         "    hr: 63,\n"
         "    avg: 0.288,\n"
         " }\n",
         {"Mark McGwire": {"avg": 0.278, "hr": 65},
          "Sammy Sosa": {"avg": 0.288, "hr": 63}}),
        # Example 2.7 Two Documents in a Stream (each with a leading comment)
        ("# Ranking of 1998 home runs\n"
         "---\n"
         "- Mark McGwire\n"
         "- Sammy Sosa\n"
         "- Ken Griffey\n"
         "\n"
         "# Team ranking\n"
         "---\n"
         "- Chicago Cubs\n"
         "- St Louis Cardinals\n",
         [["Mark McGwire", "Sammy Sosa", "Ken Griffey"],
          ["Chicago Cubs", "St Louis Cardinals"]]),
        # Example 2.8 Play by Play Feed from a Game
        ("---\n"
         "time: 20:03:20\n"
         "player: Sammy Sosa\n"
         "action: strike (miss)\n"
         "...\n"
         "---\n"
         "time: 20:03:47\n"
         "player: Sammy Sosa\n"
         "action: grand slam\n"
         "...\n",
         [{"action": "strike (miss)", "player": "Sammy Sosa", "time": "20:03:20"},
          {"action": "grand slam", "player": "Sammy Sosa", "time": "20:03:47"}]),
        # Example 2.9 Single Document with Two Comments
        ("---\n"
         "hr: # 1998 hr ranking\n"
         "- Mark McGwire\n"
         "- Sammy Sosa\n"
         "# 1998 rbi ranking\n"
         "rbi:\n"
         "- Sammy Sosa\n"
         "- Ken Griffey\n",
         {"hr": ["Mark McGwire", "Sammy Sosa"], "rbi": ["Sammy Sosa", "Ken Griffey"]}),
        # Example 2.10 Node for “Sammy Sosa” appears twice in this document
        ("---\n"
         "hr:\n"
         "- Mark McGwire\n"
         "# Following node labeled SS\n"
         "- &SS Sammy Sosa\n"
         "rbi:\n"
         "- *SS # Subsequent occurrence\n"
         "- Ken Griffey\n",
         {"hr": ["Mark McGwire", "Sammy Sosa"], "rbi": ["Sammy Sosa", "Ken Griffey"]}),
        # Example 2.12 Compact Nested Mapping
        ("---\n"
         "# Products purchased\n"
         "- item    : Super Hoop\n"
         "  quantity: 1\n"
         "- item    : Basketball\n"
         "  quantity: 4\n"
         "- item    : Big Shoes\n"
         "  quantity: 1\n",
         [{"item": "Super Hoop", "quantity": 1},
          {"item": "Basketball", "quantity": 4},
          {"item": "Big Shoes", "quantity": 1}]),
        # Example 2.13 In literals, newlines are preserved
        ("# ASCII Art\n"
         "--- |\n"
         "  \\//||\\/||\n"
         "  // ||  ||__\n",
         "\\//||\\/||\n// ||  ||__\n"),
        # Example 2.14 In the folded scalars, newlines become spaces
        ("--- >\n"
         "  Mark McGwire's\n"
         "  year was crippled\n"
         "  by a knee injury.\n",
         "Mark McGwire's year was crippled by a knee injury.\n"),
        # Example 2.16 Indentation determines scope
        ("name: Mark McGwire\n"
         "accomplishment: >\n"
         "  Mark set a major league\n"
         "  home run record in 1998.\n"
         "stats: |\n"
         "  65 Home Runs\n"
         "  0.278 Batting Average\n",
         {"accomplishment": "Mark set a major league home run record in 1998.\n",
          "name": "Mark McGwire",
          "stats": "65 Home Runs\n0.278 Batting Average\n"}),
        # Example 2.17 Quoted Scalars
        ('unicode: "Sosa did fine.\\u263A"\n'
         'control: "\\b1998\\t1999\\t2000\\n"\n'
         'hex esc: "\\x0d\\x0a is \\r\\n"\n'
         "\n"
         "single: '\"Howdy!\" he cried.'\n"
         "quoted: ' # Not a ''comment''.'\n"
         "tie-fighter: '|\\-*-/|'\n",
         {"unicode": "Sosa did fine.☺",
          "control": "\b1998\t1999\t2000\n",
          "hex esc": "\r\n is \r\n",
          "single": '"Howdy!" he cried.',
          "quoted": " # Not a 'comment'.",
          "tie-fighter": "|\\-*-/|"}),
        # Example 2.18 Multi-line Flow Scalars
        ("plain:\n"
         "  This unquoted scalar\n"
         "  spans many lines.\n"
         "\n"
         'quoted: "So does this\n'
         '  quoted scalar.\\n"\n',
         {"plain": "This unquoted scalar spans many lines.",
          "quoted": "So does this quoted scalar.\n"}),
        # Example 2.19 Integers
        ("canonical: 12345\n"
         "decimal: +12345\n"
         "octal: 0o14\n"
         "hexadecimal: 0xC\n",
         {"canonical": 12345, "decimal": 12345, "hexadecimal": 12, "octal": 12}),
        # Example 2.20 Floating Point
        ("canonical: 1.23015e+3\n"
         "exponential: 12.3015e+02\n"
         "fixed: 1230.15\n"
         "negative infinity: -.inf\n"
         "not a number: .nan\n",
         {"canonical": 1230.15,
          "exponential": 1230.15,
          "fixed": 1230.15,
          "negative infinity": float("-inf"),
          "not a number": float("nan")}),
        # Example 2.21 Miscellaneous
        ("null:\n"
         "booleans: [ true, false ]\n"
         "string: '012345'\n",
         {None: None, "booleans": [True, False], "string": "012345"}),
        # Example 2.22 Timestamps
        ("canonical: 2001-12-15T02:59:43.1Z\n"
         "iso8601: 2001-12-14t21:59:43.10-05:00\n"
         "spaced: 2001-12-14 21:59:43.10 -5\n"
         "date: 2002-12-14\n",
         {"canonical": datetime(2001, 12, 15, 2, 59, 43, 100000, tzinfo=UTC),
          "date": date(2002, 12, 14),
          "iso8601": datetime(2001, 12, 14, 21, 59, 43, 100000, tzinfo=timezone(timedelta(days=-1, seconds=68400))),
          "spaced": datetime(2001, 12, 14, 21, 59, 43, 100000, tzinfo=timezone(timedelta(days=-1, seconds=68400)))}),
        # Example 2.23 Various Explicit Tags
        ("---\n"
         "not-date: !!str 2002-04-28\n"
         "\n"
         "picture: !!binary |\n"
         " R0lGODlhDAAMAIQAAP//9/X\n"
         " 17unp5WZmZgAAAOfn515eXv\n"
         " Pz7Y6OjuDg4J+fn5OTk6enp\n"
         " 56enmleECcgggoBADs=\n"
         "\n"
         "application specific tag: !something |\n"
         " The semantics of the tag\n"
         " above may be different for\n"
         " different documents.\n",
         {"application specific tag": "The semantics of the tag\n"
                                      "above may be different for\n"
                                      "different documents.\n",
          "not-date": "2002-04-28",
          "picture": "R0lGODlhDAAMAIQAAP//9/X\n"
                     "17unp5WZmZgAAAOfn515eXv\n"
                     "Pz7Y6OjuDg4J+fn5OTk6enp\n"
                     "56enmleECcgggoBADs=\n"}),
        # Example 2.24 Global Tags
        ("%TAG ! tag:clarkevans.com,2002:\n"
         "--- !shape\n"
         "  # Use the ! handle for presenting\n"
         "  # tag:clarkevans.com,2002:circle\n"
         "- !circle\n"
         "  center: &ORIGIN {x: 73, y: 129}\n"
         "  radius: 7\n"
         "- !line\n"
         "  start: *ORIGIN\n"
         "  finish: { x: 89, y: 102 }\n"
         "- !label\n"
         "  start: *ORIGIN\n"
         "  color: 0xFFEEBB\n"
         "  text: Pretty vector drawing.\n",
         [{"center": {"x": 73, "y": 129}, "radius": 7},
          {"finish": {"x": 89, "y": 102}, "start": {"x": 73, "y": 129}},
          {"color": 16772795,
           "start": {"x": 73, "y": 129},
           "text": "Pretty vector drawing."}]),
        # Example 2.25 Unordered Sets
        ("--- !!set\n"
         "? Mark McGwire\n"
         "? Sammy Sosa\n"
         "? Ken Griffey\n",
         {"Mark McGwire", "Ken Griffey", "Sammy Sosa"}),
        # Example 2.26 Ordered Mappings
        ("--- !!omap\n"
         "- Mark McGwire: 65\n"
         "- Sammy Sosa: 63\n"
         "- Ken Griffey: 58\n",
         [{"Mark McGwire": 65}, {"Sammy Sosa": 63}, {"Ken Griffey": 58}]),
        # Example 2.27 Invoice
        ("--- !<tag:clarkevans.com,2002:invoice>\n"
         "invoice: 34843\n"
         "date   : 2001-01-23\n"
         "bill-to: &id001\n"
         "  given  : Chris\n"
         "  family : Dumars\n"
         "  address:\n"
         "    lines: |\n"
         "      458 Walkman Dr.\n"
         "      Suite #292\n"
         "    city    : Royal Oak\n"
         "    state   : MI\n"
         "    postal  : 48046\n"
         "ship-to: *id001\n"
         "product:\n"
         "- sku         : BL394D\n"
         "  quantity    : 4\n"
         "  description : Basketball\n"
         "  price       : 450.00\n"
         "- sku         : BL4438H\n"
         "  quantity    : 1\n"
         "  description : Super Hoop\n"
         "  price       : 2392.00\n"
         "tax  : 251.42\n"
         "total: 4443.52\n"
         "comments:\n"
         "  Late afternoon is best.\n"
         "  Backup contact is Nancy\n"
         "  Billsmer @ 338-4338.\n",
         {"bill-to": {"address": {"city": "Royal Oak",
                                  "lines": "458 Walkman Dr.\nSuite #292\n",
                                  "postal": 48046,
                                  "state": "MI"},
                      "family": "Dumars",
                      "given": "Chris"},
          "comments": "Late afternoon is best. Backup contact is Nancy Billsmer @ "
                      "338-4338.",
          "date": date(2001, 1, 23),
          "invoice": 34843,
          "product": [{"description": "Basketball",
                       "price": 450.0,
                       "quantity": 4,
                       "sku": "BL394D"},
                      {"description": "Super Hoop",
                       "price": 2392.0,
                       "quantity": 1,
                       "sku": "BL4438H"}],
          "ship-to": {"address": {"city": "Royal Oak",
                                  "lines": "458 Walkman Dr.\nSuite #292\n",
                                  "postal": 48046,
                                  "state": "MI"},
                      "family": "Dumars",
                      "given": "Chris"},
          "tax": 251.42,
          "total": 4443.52}),
        # Example 2.28 Log File
        ("---\n"
         "Time: 2001-11-23 15:01:42 -5\n"
         "User: ed\n"
         "Warning:\n"
         "  This is an error message\n"
         "  for the log file\n"
         "---\n"
         "Time: 2001-11-23 15:02:31 -5\n"
         "User: ed\n"
         "Warning:\n"
         "  A slightly different error\n"
         "  message.\n"
         "---\n"
         "Date: 2001-11-23 15:03:17 -5\n"
         "User: ed\n"
         "Fatal:\n"
         '  Unknown variable "bar"\n'
         "Stack:\n"
         "- file: TopClass.py\n"
         "  line: 23\n"
         "  code: |\n"
         '    x = MoreObject("345\\n")\n'
         "- file: MoreClass.py\n"
         "  line: 58\n"
         "  code: |-\n"
         "    foo = bar\n",
         [{"Time": datetime(2001, 11, 23, 15, 1, 42, tzinfo=timezone(timedelta(days=-1, seconds=68400))),
           "User": "ed",
           "Warning": "This is an error message for the log file"},
          {"Time": datetime(2001, 11, 23, 15, 2, 31, tzinfo=timezone(timedelta(days=-1, seconds=68400))),
           "User": "ed",
           "Warning": "A slightly different error message."},
          {"Date": datetime(2001, 11, 23, 15, 3, 17, tzinfo=timezone(timedelta(days=-1, seconds=68400))),
           "Fatal": 'Unknown variable "bar"',
           "Stack": [{"code": 'x = MoreObject("345\\n")\n',
                      "file": "TopClass.py",
                      "line": 23},
                     {"code": "foo = bar", "file": "MoreClass.py", "line": 58}],
           "User": "ed"}]),
        # Example 10.1 !!map Examples
        ("Block style: !!map\n"
         "  Clark : Evans\n"
         "  Ingy  : döt Net\n"
         "  Oren  : Ben-Kiki\n"
         "\n"
         "Flow style: !!map { Clark: Evans, Ingy: döt Net, Oren: Ben-Kiki }\n",
         {"Block style": {"Clark": "Evans", "Ingy": "döt Net", "Oren": "Ben-Kiki"},
          "Flow style": {"Clark": "Evans", "Ingy": "döt Net", "Oren": "Ben-Kiki"}}),
        # Example 10.2 !!seq Examples
        ("Block style: !!seq\n"
         "- Clark Evans\n"
         "- Ingy döt Net\n"
         "- Oren Ben-Kiki\n"
         "\n"
         "Flow style: !!seq [ Clark Evans, Ingy döt Net, Oren Ben-Kiki ]\n",
         {"Block style": ["Clark Evans", "Ingy döt Net", "Oren Ben-Kiki"],
          "Flow style": ["Clark Evans", "Ingy döt Net", "Oren Ben-Kiki"]}),
        # Example 10.3 !!str Examples
        ("Block style: !!str |-\n"
         "  String: just a theory.\n"
         "\n"
         'Flow style: !!str "String: just a theory."\n',
         {"Block style": "String: just a theory.",
          "Flow style": "String: just a theory."}),
        # Example 10.4 !!null Examples
        ("!!null null: value for null key\n"
         "key with null value: !!null null",
         {None: "value for null key", "key with null value": None}),
        # Example 10.5 !!bool Examples
        ("YAML is a superset of JSON: !!bool true\n"
         "Pluto is a planet: !!bool false",
         {"Pluto is a planet": False, "YAML is a superset of JSON": True}),
        # Example 10.6 !!int Examples
        ("negative: !!int -12\n"
         "zero: !!int 0\n"
         "positive: !!int 34\n",
         {"negative": -12, "positive": 34, "zero": 0}),
        # Example 10.7 !!float Examples
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
    ],
)
# fmt off
def test_parse(yaml: str, parsed: Any) -> None:
    assert yaml_rs.loads(yaml) == _is_nan(parsed)


@pytest.mark.parametrize("yaml", VALID_YAMLS)
def test_valid_yamls_from_test_suite(yaml: Path) -> None:
    load_from_str = yaml_rs.loads(yaml.read_text(encoding="utf-8"), parse_datetime=False)

    docs = [load_from_str] if isinstance(load_from_str, dict) else load_from_str

    for doc in docs:
        parsed_yaml = yaml_rs.loads(normalize_yaml(doc), parse_datetime=False)
        if isinstance(parsed_yaml, set):
            parsed_yaml = dict.fromkeys(parsed_yaml)

        get_json_key = doc.get("json")

        if get_json_key is None:
            assert parsed_yaml is not None
            continue

        if get_json_key == "":  # noqa: PLC1901
            get_json_key = None
            continue

        try:
            parsed_json = json.loads(get_json_key)
        except json.decoder.JSONDecodeError:
            json_decoder = json.JSONDecoder()
            parsed_json = []
            pos = 0
            while pos < len(get_json_key):
                obj, pos = json_decoder.raw_decode(get_json_key, pos)
                parsed_json.append(obj)
                while pos < len(get_json_key) and get_json_key[pos] in " \t\n\r":
                    pos += 1

            if len(parsed_json) == 1:
                parsed_json = parsed_json[0]

        assert parsed_yaml == parsed_json


@pytest.mark.parametrize("yaml", INVALID_YAMLS)
def test_invalid_yamls_from_test_suite(yaml: Path) -> None:
    load_from_str = yaml_rs.loads(yaml.read_text(encoding="utf-8"), parse_datetime=False)
    docs = load_from_str if isinstance(load_from_str, list) else [load_from_str]
    doc = next((d for d in docs if d.get("fail") is True), None)
    with pytest.raises(yaml_rs.YAMLDecodeError):
        yaml_rs.loads(normalize_yaml(doc), parse_datetime=False)
