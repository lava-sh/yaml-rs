import time
from collections.abc import Callable
from pathlib import Path

import yaml_rs
from rich.console import Console
from rich.table import Table

import os
print("CWD:", os.getcwd())
print("Repo root contents:", os.listdir(os.getcwd()))

ROOT = Path(__file__).resolve().parents[1]
YAMLS = ROOT / "benchmark" / "data"
# example of a config file for app
FILE_1 = YAMLS / "config.yaml"
# file from https://github.com/yaml/yaml-test-suite
FILE_2 = YAMLS / "UGM3.yaml"
# file from `https://examplefile.com`
FILE_3 = YAMLS / "bench.yaml"

N = 300

def benchmark(func: Callable, count: int) -> float:
    start = time.perf_counter()
    for _ in range(count):
        func()
    end = time.perf_counter()
    return end - start


def read_yaml(path: Path) -> str:
    return path.read_text(encoding="utf-8")


tests = {
    "FILE_1 loads": lambda: yaml_rs.loads(read_yaml(FILE_1)),
    "FILE_2 loads": lambda: yaml_rs.loads(read_yaml(FILE_2)),
    "FILE_3 loads": lambda: yaml_rs.loads(read_yaml(FILE_3)),
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
