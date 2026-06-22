# To run the benchmarks

## Create and activate virtual environment

<p>
  <img
    src="https://thesvg.org/icons/linux/default.svg"
    alt="linux"
    height="14"
  />
  Linux /
  <img
    src="https://thesvg.org/icons/apple/default.svg"
    alt="macos"
    height="14"
  />
  MacOS:
</p>

```bash
python3 -m venv .venv
source .venv/bin/activate
```

<p>
  <img
    src="https://thesvg.org/icons/windows11/default.svg"
    alt="windows"
    height="14"
  />
  Windows:
</p>

```bash
py -m venv .venv
.venv\scripts\activate
```

## Install benchmark dependencies

<p>
  <img
    src="https://thesvg.org/icons/python/default.svg"
    alt="Python"
    height="14"
  />
  Using <a href="https://github.com/pypa/pip">pip</a>:
</p>

```bash
pip install . --group bench
```

<p>
  <img
    src="https://thesvg.org/icons/uv/default.svg"
    alt="uv"
    height="14"
  />
  Using <a href="https://github.com/astral-sh/uv">uv</a>:
</p>

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
