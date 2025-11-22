import time
from collections.abc import Callable

import yaml_rs
from rich.console import Console
from rich.table import Table

YAML_1 = """\
app:
  local: true
  logging:
    level: INFO
  version: 1.7
  release-date: 2015-07-09

  mysql:
    user: "user"
    password: "password"
    host: "127.0.0.1"
    port: 3306
    db_name: "database"
"""
YAML_2 = """\
---
- name: Spec Example 2.25. Unordered Sets
  from: http://www.yaml.org/spec/1.2/spec.html#id2761758
  tags: spec mapping unknown-tag explicit-key
  yaml: |
    # Sets are represented as a
    # Mapping where each key is
    # associated with a null value
    --- !!set
    ? Mark McGwire
    ? Sammy Sosa
    ? Ken Griff
  tree: |
    +STR
     +DOC ---
      +MAP <tag:yaml.org,2002:set>
       =VAL :Mark McGwire
       =VAL :
       =VAL :Sammy Sosa
       =VAL :
       =VAL :Ken Griff
       =VAL :
      -MAP
     -DOC
    -STR
  json: |
    {
      "Mark McGwire": null,
      "Sammy Sosa": null,
      "Ken Griff": null
    }
  dump: |
    --- !!set
    Mark McGwire:
    Sammy Sosa:
    Ken Griff:
"""
YAML_3 = """\
---
- name: Spec Example 7.10. Plain Characters
  from: http://www.yaml.org/spec/1.2/spec.html#id2789510
  tags: spec flow sequence scalar
  yaml: |
    # Outside flow collection:
    - ::vector
    - ": - ()"
    - Up, up, and away!
    - -123
    - http://example.com/foo#bar
    # Inside flow collection:
    - [ ::vector,
      ": - ()",
      "Up, up and away!",
      -123,
      http://example.com/foo#bar ]
  tree: |
    +STR
     +DOC
      +SEQ
       =VAL :::vector
       =VAL ": - ()
       =VAL :Up, up, and away!
       =VAL :-123
       =VAL :http://example.com/foo#bar
       +SEQ []
        =VAL :::vector
        =VAL ": - ()
        =VAL "Up, up and away!
        =VAL :-123
        =VAL :http://example.com/foo#bar
       -SEQ
      -SEQ
     -DOC
    -STR
  json: |
    [
      "::vector",
      ": - ()",
      "Up, up, and away!",
      -123,
      "http://example.com/foo#bar",
      [
        "::vector",
        ": - ()",
        "Up, up and away!",
        -123,
        "http://example.com/foo#bar"
      ]
    ]
  dump: |
    - ::vector
    - ": - ()"
    - Up, up, and away!
    - -123
    - http://example.com/foo#bar
    - - ::vector
      - ": - ()"
      - "Up, up and away!"
      - -123
      - http://example.com/foo#bar
"""

N = 100_000


def benchmark(func: Callable, count: int) -> float:
    start = time.perf_counter()
    for _ in range(count):
        func()
    end = time.perf_counter()
    return end - start


tests = {
    "YAML_1 loads": lambda: yaml_rs.loads(YAML_1),
    "YAML_2 loads": lambda: yaml_rs.loads(YAML_2),
    "YAML_3 loads": lambda: yaml_rs.loads(YAML_3),
}

console = Console()
table = Table(
    title="Benchmark",
    show_lines=True,
    header_style="bold magenta",
)
table.add_column("Test", justify="center")
table.add_column("Iterations", justify="center")
table.add_column("Time", justify="center")

for name, fn in tests.items():
    for _ in range(30):
        fn()
    t = benchmark(fn, N)
    table.add_row(name, f"{N}", f"{t:.6f}s")

console.print(table)
