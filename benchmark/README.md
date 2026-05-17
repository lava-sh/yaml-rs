# To run the benchmarks

## Create and activate virtual environment

Linux / MacOS:

```bash
python3 -m venv .venv
source .venv/bin/activate
```

Windows:

```bash
py -m venv .venv
.venv\scripts\activate
```

## Install benchmark dependencies

Using [pip](https://github.com/pypa/pip):

```bash
pip install . --group bench
```

Using [uv](https://github.com/astral-sh/uv):

```bash
uv pip install . --group bench
```

## Run `benchmark/run.py`

```bash
python benchmark/run.py
```

## Results

### loads

![YAML loads benchmark](loads.svg)

### dumps

![YAML dumps benchmark](dumps.svg)
