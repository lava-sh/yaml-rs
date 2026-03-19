<div align="center">

# yaml-rs

*A High-Performance YAML parser for Python written in Rust*

| 🐍 PyPI                                                                                          | 🐙 GitHub                                                                                                |
|--------------------------------------------------------------------------------------------------|----------------------------------------------------------------------------------------------------------|
| ![License](https://img.shields.io/pypi/l/yaml_rs?style=flat-square&color=007ec6)                 | ![Stars](https://img.shields.io/github/stars/lava-sh/yaml-rs?style=flat-square&color=007ec6)             |
| ![Downloads](https://img.shields.io/pypi/dm/yaml_rs?style=flat-square&color=007ec6)              | ![Repo size](https://img.shields.io/github/repo-size/lava-sh/yaml-rs?style=flat-square&color=007ec6)     |
| ![Python Version](https://img.shields.io/pypi/pyversions/yaml_rs?style=flat-square&color=007ec6) | ![Last Commit](https://img.shields.io/github/last-commit/lava-sh/yaml-rs?style=flat-square&color=007ec6) |

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

## Why not [pyyaml](https://pypi.org/project/PyYAML), [ruamel.yaml](https://pypi.org/project/ruamel.yaml), [strictyaml](https://pypi.org/project/strictyaml)?

`PyYAML` and `ruamel.yaml` сan't parse example 2.23, 2.24, 2.27, 2.28, etc.
from [YAML spec](https://yaml.org/spec/1.2.2)
and also do not pass all tests from [yaml-test-suite](https://github.com/yaml/yaml-test-suite).

`strictyaml` use `ruamel.yaml` as parser so all the bugs are repeated too.

```python
import yaml as pyyaml

example_2_23 = """\
---
not-date: !!str 2002-04-28

picture: !!binary |
 R0lGODlhDAAMAIQAAP//9/X
 17unp5WZmZgAAAOfn515eXv
 Pz7Y6OjuDg4J+fn5OTk6enp
 56enmleECcgggoBADs=

application specific tag: !something |
 The semantics of the tag
 above may be different for
 different documents.
"""
print(pyyaml.safe_load(example_2_23))  # yaml.constructor.ConstructorError
```

```python
import yaml as pyyaml
from ruamel.yaml import YAML

yaml_safe = YAML(typ="safe")

yaml = "! 15"  # must be str

pyyaml_load = pyyaml.safe_load(yaml)
ruamel_yaml_load = yaml_safe.load(yaml)
print(pyyaml_load)  # 15
print(type(pyyaml_load))  # <class 'int'>
print(ruamel_yaml_load)  # 15
print(type(ruamel_yaml_load))  # <class 'int'>
```
