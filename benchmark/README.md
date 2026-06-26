# To run the benchmarks

## Create and activate virtual environment

<p>
  <span style="white-space: nowrap;">
    <img
      src="https://thesvg.org/icons/linux/default.svg"
      alt="linux"
      height="14"
    />
    Linux /
    <picture>
      <source
        media="(prefers-color-scheme: dark)"
        srcset="https://thesvg.org/icons/apple/default.svg"
      />
      <img
        src="https://thesvg.org/icons/apple/mono.svg"
        alt="macos"
        height="14"
      />
    </picture>
    MacOS:
  </span>
</p>

```bash
python3 -m venv .venv
source .venv/bin/activate
```

<p>
  <img
    src="https://thesvg.org/icons/windows/default.svg"
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

Benchmarks are updated daily and stored in [this](https://github.com/lava-sh/benchmarks/tree/main/yaml-rs) repository.

<details>
<summary>
<img src="https://thesvg.org/icons/linux/default.svg" height="16" />
Linux (click me)
</summary>

<img src="https://github.com/lava-sh/benchmarks/blob/main/yaml-rs/ubuntu-loads.svg">
<img src="https://github.com/lava-sh/benchmarks/blob/main/yaml-rs/ubuntu-dumps.svg">

</details>

<details>
<summary>
<picture>
  <source
    media="(prefers-color-scheme: dark)"
    srcset="https://thesvg.org/icons/apple/default.svg"
  />
  <img
    src="https://thesvg.org/icons/apple/mono.svg"
    height="16"
  />
</picture>
macOS (click me)
</summary>

<img src="https://github.com/lava-sh/benchmarks/blob/main/yaml-rs/macos-loads.svg">
<img src="https://github.com/lava-sh/benchmarks/blob/main/yaml-rs/macos-loads.svg">

</details>

<details>
<summary>
<img src="https://thesvg.org/icons/windows/default.svg" height="16" />
Windows (click me)
</summary>

<img src="https://github.com/lava-sh/benchmarks/blob/main/yaml-rs/windows-loads.svg">
<img src="https://github.com/lava-sh/benchmarks/blob/main/yaml-rs/windows-dumps.svg">

</details>
