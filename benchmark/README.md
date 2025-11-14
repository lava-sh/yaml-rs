# To run the benchmarks

## Create and activate virtual environment

```bash
# Linux / MacOS
python3 -m venv .venv
source .venv/bin/activate

# Windows
py -m venv .venv
.venv\scripts\activate
```

## Install benchmark dependencies

```bash
# Using pip
pip install . --group bench

# Using uv
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
