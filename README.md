<div align="center">

# yaml-rs

*A High-Performance YAML parser for Python written in Rust*

[![PyPI License](https://img.shields.io/pypi/l/yaml_rs.svg?style=flat-square)](https://pypi.org/project/yaml_rs/)
[![Python version](https://img.shields.io/pypi/pyversions/yaml_rs.svg?style=flat-square)](https://pypi.org/project/yaml_rs/)
[![Implementation](https://img.shields.io/pypi/implementation/yaml_rs.svg?style=flat-square)](https://pypi.org/project/yaml_rs/)

[![Monthly downloads](https://img.shields.io/pypi/dm/yaml_rs.svg?style=)](https://pypi.org/project/yaml_rs/)
[![Github Repository size](https://img.shields.io/github/repo-size/lava-sh/yaml-rs?style=flat-square)](https://github.com/lava-sh/yaml-rs)

</div>

## Features

* The fastest YAML parser in Python (see [benchmarks](https://github.com/lava-sh/yaml-rs/tree/main/benchmark))

## Installation

```bash
# Using pip
pip install yaml-rs

# Using uv
uv pip install yaml-rs
```

## Examples

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
