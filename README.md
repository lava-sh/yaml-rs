<div align="center">

# yaml-rs

*A High-Performance YAML v1.2 parser for Python written in Rust*

| 🐍 PyPI                                                                                          | 🐙 GitHub                                                                                             |
|--------------------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------------------|
| ![Version](https://img.shields.io/pypi/v/yaml_rs?style=flat-square&color=007ec6)                 | ![Stars](https://img.shields.io/github/stars/lava-sh/yaml-rs?style=flat-square&color=007ec6)          |
| ![License](https://img.shields.io/pypi/l/yaml_rs?style=flat-square&color=007ec6)                 | ![CI](https://img.shields.io/github/actions/workflow/status/lava-sh/yaml-rs/ci.yml?style=flat-square) |
| ![Downloads](https://img.shields.io/pypi/dm/yaml_rs?style=flat-square&color=007ec6)              | ![Repo size](https://img.shields.io/github/repo-size/lava-sh/yaml-rs?style=flat-square&color=007ec6)  |
| ![Python Version](https://img.shields.io/pypi/pyversions/yaml_rs?style=flat-square&color=007ec6) | ![Last Commit](https://img.shields.io/github/last-commit/lava-sh/yaml-rs?style=flat-square)           |

</div>

## Features

* The fastest YAML parser in Python (see [benchmarks](https://github.com/lava-sh/yaml-rs/tree/main/benchmark))
* Full YAML v1.2 spec support

## Installation

```bash
# Using pip
pip install yaml-rs

# Using uv
uv pip install yaml-rs
```

## [Playground](https://lava-sh.github.io/yaml-rs-online/)

Link: <https://lava-sh.github.io/yaml-rs-online/>

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

![GitHub last commit](https://img.shields.io/github/last-commit/yaml/pyyaml?style=flat-square)

`PyYAML` is a parser for [YAML 1.1](https://github.com/yaml/pyyaml/blob/6.0.3/setup.py#L10)

It does [not pass](https://matrix.yaml.info) the [yaml-test-suite](https://github.com/yaml/yaml-test-suite).

#### [oyaml](https://pypi.org/project/oyaml)

![GitHub last commit](https://img.shields.io/github/last-commit/wimglenn/oyaml?style=flat-square)

`oyaml`
is [Ordered YAML: drop-in replacement for PyYAML which preserves dict ordering](https://github.com/wimglenn/oyaml).

Because it is a fork of `PyYAML`, it has the same problems.

#### [ryaml](https://pypi.org/project/ryaml)

![GitHub last commit](https://img.shields.io/github/last-commit/emmatyping/ryaml?style=flat-square)

`ryaml` is a parser with a Rust core focused on compatibility with `PyYAML`.

### YAML 1.2-oriented libraries

#### [ruamel.yaml](https://pypi.org/project/ruamel.yaml)

`ruamel.yaml` is a [YAML 1.2 parser/emitter for Python](https://sourceforge.net/projects/ruamel-yaml).

It supports round-trip preservation of comments, sequence and mapping flow style, and mapping key order.

However, it does [not pass](https://matrix.yaml.info) the [yaml-test-suite](https://github.com/yaml/yaml-test-suite).

#### [strictyaml](https://pypi.org/project/strictyaml)

![GitHub last commit](https://img.shields.io/github/last-commit/crdoconnor/strictyaml?style=flat-square)

`strictyaml` is a [Type-safe YAML parser and validator](https://github.com/crdoconnor/strictyaml).

It also does not pass the [yaml-test-suite](https://github.com/yaml/yaml-test-suite).
