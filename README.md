<!-- rumdl-disable MD036 MD041-->
<div align="center">

# yaml-rs

_A High-Performance YAML v1.2 parser for Python written in Rust_
<!-- rumdl-enable MD036 MD041-->

![PyPI version](https://shieldcn.dev/badge/dynamic/json.svg?url=https%3A%2F%2Fpypi.org%2Fpypi%2Fyaml-rs%2Fjson&query=%24.info.version&variant=branded&size=xs&mode=light&logo=python&label=pypi+version)
![PyPI downloads](https://shieldcn.dev/badge/dynamic/json.svg?url=https%3A%2F%2Fpypistats.org%2Fapi%2Fpackages%2Fyaml-rs%2Frecent&query=%24.data.last_month&suffix=%2Fmonth&size=xs&mode=light&logo=python&logoColor=ffffff&label=downloads&color=3775A9)
![PyPI requires python](https://shieldcn.dev/badge/dynamic/json.svg?url=https%3A%2F%2Fpypi.org%2Fpypi%2Fyaml-rs%2Fjson&query=%24.info.requires_python&size=xs&mode=light&logo=python&logoColor=ffffff&label=requires+python&color=3775A9)

<a href="https://github.com/lava-sh/yaml-rs/actions?query=branch%3Amain"><picture><source media="(prefers-color-scheme: dark)" srcset="https://shieldcn.dev/github/ci/lava-sh/yaml-rs.svg?variant=outline&font=geist-mono&size=xs&animate=pulse&mode=dark"><img alt="CI" src="https://shieldcn.dev/github/ci/lava-sh/yaml-rs.svg?variant=outline&font=geist-mono&size=xs&animate=pulse&mode=light"></picture></a>
<a href="https://github.com/lava-sh/yaml-rs/commits/main"><picture><source media="(prefers-color-scheme: dark)" srcset="https://shieldcn.dev/github/last-commit/lava-sh/yaml-rs.svg?variant=outline&font=geist-mono&size=xs&mode=dark"><img alt="Last Commit" src="https://shieldcn.dev/github/last-commit/lava-sh/yaml-rs.svg?variant=outline&font=geist-mono&size=xs&mode=light"></picture></a>
<a href="https://github.com/lava-sh/yaml-rs/blob/main/UNLICENSE"><picture><source media="(prefers-color-scheme: dark)" srcset="https://shieldcn.dev/github/lava-sh/yaml-rs/license.svg?variant=outline&font=geist-mono&size=xs&mode=dark"><img alt="License" src="https://shieldcn.dev/github/lava-sh/yaml-rs/license.svg?variant=outline&font=geist-mono&size=xs&mode=light"></picture></a>

</div>

## Features

* The fastest YAML parser in Python (see [benchmarks](https://github.com/lava-sh/yaml-rs/tree/main/benchmark))
* Full YAML v1.2 spec support

## Installation

<p>
  <img
    src="https://thesvg.org/icons/python/default.svg"
    alt="Python"
    height="14"
  />
  Using <a href="https://github.com/pypa/pip">pip</a>:
</p>

```bash
pip install yaml-rs
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
uv pip install yaml-rs
```

<p>
  <img
    src="https://thesvg.org/icons/poetry/default.svg"
    alt="Poetry"
    height="14"
  />
  Using <a href="https://github.com/python-poetry/poetry">poetry</a>:
</p>

```bash
poetry add yaml-rs
```

## [Playground]

Link: <https://lava-sh.github.io/yaml-rs-online>

## Example

```python
from pprint import pprint

import yaml_rs

yaml = """\
app:
  name: service
  environment: production
  debug: false
  version: 1.3.5

  log:
    level: INFO
    file: /var/log/service/app.log
    rotation:
      enabled: true
      max_size_mb: 50

  database:
    engine: mariadb
    host: localhost
    port: 3306
    username: app_user
    password: super_secret_password
    pool_size: 10
    timeout_seconds: 30

  metadata:
    author: "John Doe"
    created_at: 2024-01-15T12:00:00Z
    updated_at: 2025-11-09T10:30:00Z
"""
pprint(yaml_rs.loads(yaml))
```

## Comparison with other YAML parsing libraries

> [!NOTE]
> Information current as of March 19, 2026.

### YAML 1.1-oriented libraries

#### [PyYAML](https://pypi.org/project/PyYAML)

<a href="https://github.com/yaml/pyyaml/commits/main"><picture><source media="(prefers-color-scheme: dark)" srcset="https://shieldcn.dev/github/last-commit/yaml/pyyaml.svg?variant=outline&font=geist-mono&size=xs&mode=dark"><img alt="Last Commit" src="https://shieldcn.dev/github/last-commit/yaml/pyyaml.svg?variant=outline&font=geist-mono&size=xs&mode=light"></picture></a>

`PyYAML` is a parser for [YAML 1.1](https://github.com/yaml/pyyaml/blob/6.0.3/setup.py#L10)

It does [not pass](https://matrix.yaml.info) the [yaml-test-suite](https://github.com/yaml/yaml-test-suite).

#### [oyaml](https://pypi.org/project/oyaml)

<a href="https://github.com/wimglenn/oyaml/commits/main"><picture><source media="(prefers-color-scheme: dark)" srcset="https://shieldcn.dev/github/last-commit/wimglenn/oyaml.svg?variant=outline&font=geist-mono&size=xs&mode=dark"><img alt="Last Commit" src="https://shieldcn.dev/github/last-commit/wimglenn/oyaml.svg?variant=outline&font=geist-mono&size=xs&mode=light"></picture></a>

`oyaml`
is [Ordered YAML: drop-in replacement for PyYAML which preserves dict ordering](https://github.com/wimglenn/oyaml).

Because it is a fork of `PyYAML`, it has the same problems.

#### [ryaml](https://pypi.org/project/ryaml)

<a href="https://github.com/emmatyping/ryaml/commits/main"><picture><source media="(prefers-color-scheme: dark)" srcset="https://shieldcn.dev/github/last-commit/emmatyping/ryaml.svg?variant=outline&font=geist-mono&size=xs&mode=dark"><img alt="Last Commit" src="https://shieldcn.dev/github/last-commit/emmatyping/ryaml.svg?variant=outline&font=geist-mono&size=xs&mode=light"></picture></a>

`ryaml` is a parser with a Rust core focused on compatibility with `PyYAML`.

### YAML 1.2-oriented libraries

#### [ruamel.yaml](https://pypi.org/project/ruamel.yaml)

`ruamel.yaml` is a [YAML 1.2 parser/emitter for Python](https://sourceforge.net/projects/ruamel-yaml).

It supports round-trip preservation of comments, sequence and
mapping flow style, and mapping key order.

However, it does [not pass](https://matrix.yaml.info) the [yaml-test-suite](https://github.com/yaml/yaml-test-suite).

#### [strictyaml](https://pypi.org/project/strictyaml)

<a href="https://github.com/crdoconnor/strictyaml/commits/main"><picture><source media="(prefers-color-scheme: dark)" srcset="https://shieldcn.dev/github/last-commit/crdoconnor/strictyaml.svg?variant=outline&font=geist-mono&size=xs&mode=dark"><img alt="Last Commit" src="https://shieldcn.dev/github/last-commit/crdoconnor/strictyaml.svg?variant=outline&font=geist-mono&size=xs&mode=light"></picture></a>

`strictyaml` is a [Type-safe YAML parser and validator](https://github.com/crdoconnor/strictyaml).

It also does not pass the [yaml-test-suite](https://github.com/yaml/yaml-test-suite).

<div align="center">

## Contributors

[![lava-sh/toml-rs contributors](https://shieldcn.dev/contributors/lava-sh/yaml-rs.svg?title=false&theme=slate&size=80&bots=true&titleAlign=center&mode=light&font=geist&border=false&image=https%3A%2F%2Fimages.wallpaperscraft.ru%2Fimage%2Fsingle%2Foblaka_nebo_ogni_1647475_3840x2400.jpg&overlay=0.3)](https://github.com/lava-sh/toml-rs/graphs/contributors)

</div>

[Playground]: https://lava-sh.github.io/yaml-rs-online
