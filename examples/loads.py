from pprint import pprint

import yaml_rs

yaml = """\
app:
  local: True
  logging:
    level: INFO
  version: 1.7
  release-date: 2015-07-09

  mysql:
    user: "user"
    password: "password"
    host: "127.0.0.1"
    port: 3306
    db_name: "database"
"""
# by default `yaml_rs.loads(...)` parse date
pprint(yaml_rs.loads(yaml))
# also, you can disable this behavior
print("\nyaml_rs.loads(..., parse_datetime=False):")
loads = yaml_rs.loads(yaml, parse_datetime=False)
pprint(loads)
print(type(loads["app"]["release-date"]))  # <class 'str'>
