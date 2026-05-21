__all__ = (
    "UTC",
    "dt",
    "is_nan",
    "tzinfo",
)

import datetime
import math
import sys
from typing import Any

from dirty_equals import IsFloatNan

if sys.version_info >= (3, 11):
    from datetime import UTC
else:
    import datetime

    UTC = datetime.timezone.utc

tzinfo = datetime.timezone(datetime.timedelta(days=-1, seconds=68400))
dt = datetime.datetime(
    2001,
    12,
    14,
    21,
    59,
    43,
    100000,
    tzinfo=tzinfo,
)


def is_nan(obj: Any) -> Any | dict[Any, Any] | list[Any] | IsFloatNan:
    if isinstance(obj, dict):
        return {k: is_nan(v) for k, v in obj.items()}
    if isinstance(obj, list):
        return [is_nan(v) for v in obj]
    if isinstance(obj, float) and math.isnan(obj):
        return IsFloatNan
    return obj
