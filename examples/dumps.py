import datetime
from pathlib import Path

import yaml_rs

data = {
    "app": {
        "local": True,
        "logging": {"level": "INFO"},
        "release-date": datetime.date(2015, 7, 9),
        "version": 1.7,
        "mysql": {
            "db_name": "database",
            "host": "127.0.0.1",
            "password": "password",
            "port": 3306,
            "user": "user",
        },
    },
}
print(yaml_rs.dumps(data))

loads = yaml_rs.loads(Path("config.yaml").read_text("utf-8"))
print(yaml_rs.dumps(loads))
