from datetime import date, datetime, time, timedelta, timezone
from decimal import Decimal
from io import StringIO
from pathlib import Path
from tempfile import TemporaryDirectory
from typing import Any

import yaml_rs

ROOT = Path(__file__).resolve().parent


def build_obj() -> dict[str, Any]:
    return {
        "title": "PGO profile",
        "numbers": list(range(64)),
        "tuple": tuple(range(16)),
        "nested": {
            "date": date(1979, 5, 27),
            "time": time(7, 32),
            "datetime": datetime(
                1979,
                5,
                27,
                7,
                32,
                tzinfo=timezone(timedelta(hours=-8)),
            ),
            "decimal": Decimal("12345.6789"),
            "items": [
                {"value": Decimal("1.50")},
                {"value": Decimal("1E+3")},
                {"value": Decimal("Infinity")},
                {"value": Decimal("sNaN")},
            ],
        },
    }


def main() -> None:
    file = ROOT / "data" / "example.yaml"
    text = file.read_text(encoding="utf-8")
    obj = build_obj()

    for _ in range(4000):
        yaml_rs.loads(text)

    for _ in range(3000):
        yaml_rs.dumps(obj)

    for _ in range(2000):
        dumped = yaml_rs.dumps(obj)
        yaml_rs.loads(dumped)

    for _ in range(1500):
        buffer = StringIO()
        yaml_rs.dump(obj, buffer)
        yaml_rs.loads(buffer.getvalue())

    with TemporaryDirectory() as tmp_dir:
        tmp_path = Path(tmp_dir) / "profile.yaml"

        for _ in range(1500):
            yaml_rs.dump(obj, tmp_path)
            with tmp_path.open("rb") as fp:
                yaml_rs.load(fp)


if __name__ == "__main__":
    main()
