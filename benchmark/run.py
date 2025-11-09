import platform
import time
from collections.abc import Callable
from pathlib import Path

import altair as alt
import cpuinfo
import oyaml
import polars as pl
import ruamel.yaml
import strictyaml
import yaml as pyyaml
import yaml_rs

N = 2500


def benchmark(func: Callable, count: int) -> float:
    start = time.perf_counter()
    for _ in range(count):
        func()
    end = time.perf_counter()
    return end - start


def plot_benchmark(results: dict[str, float], save_path: Path) -> None:
    df = (
        pl.DataFrame({
            "parser": list(results.keys()),
            "exec_time": list(results.values()),
        })
        .sort("exec_time")
        .with_columns([
            (pl.col("exec_time") / pl.col("exec_time").min()).alias("slowdown"),
        ])
    )

    chart = (
        alt.Chart(df)
        .mark_bar(cornerRadiusTopLeft=6, cornerRadiusTopRight=6)
        .encode(
            x=alt.X("parser:N", sort=None, title="Parser", axis=alt.Axis(labelAngle=0)),
            y=alt.Y(
                "exec_time:Q",
                title="Execution Time (seconds, lower=better)",
                scale=alt.Scale(domain=(0, df["exec_time"].max() * 1.04)),
                axis=alt.Axis(grid=False),
            ),
            color=alt.Color("parser:N", legend=None, scale=alt.Scale(scheme="dark2")),
            tooltip=[
                alt.Tooltip("parser:N", title=""),
                alt.Tooltip("exec_time:Q", title="Execution Time (s)", format=".4f"),
                alt.Tooltip("slowdown:Q", title="Slowdown", format=".2f"),
            ],
        )
    )

    text = (
        chart.mark_text(
            align="center",
            baseline="bottom",
            dy=-2,
            fontSize=9,
            fontWeight="bold",
        )
        .transform_calculate(
            label='format(datum.exec_time, ".4f") + '
            '"s (x" + format(datum.slowdown, ".2f") + ")"',
        )
        .encode(text="label:N")
    )

    os = f"{platform.system()} {platform.release()}"
    cpu = cpuinfo.get_cpu_info()["brand_raw"]
    py = platform.python_version()
    (chart + text).properties(
        width=600,
        height=400,
        title={
            "text": "YAML parsers benchmark (loads)",
            "subtitle": f"Python: {py} ({os}) | CPU: {cpu}",
        },
    ).save(save_path)


file = Path(__file__).resolve().parent
bench_yaml = file.parent / "tests" / "data" / "bench.yaml"
data = bench_yaml.read_text(encoding="utf-8")


def run(run_count: int) -> None:
    loads = {
        "yaml_rs": lambda: yaml_rs.loads(data),
        "yaml_rs (parse_dt=False)": lambda: yaml_rs.loads(data, parse_datetime=False),
        "PyYAML": lambda: pyyaml.safe_load(data),
        "ruamel.yaml": lambda: ruamel.yaml.YAML(typ="safe").load(data),
        "oyaml": lambda: oyaml.safe_load(data),
        "strictyaml": lambda: strictyaml.load(data),
    }
    results = {name: benchmark(func, run_count) for name, func in loads.items()}
    plot_benchmark(results, save_path=file / "loads.svg")


if __name__ == "__main__":
    run(N)
