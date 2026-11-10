"""Test-only support for existing mocked paired harness contracts."""
from functools import wraps
from unittest.mock import patch


def mocked_native_preflight(test):
    """Exercise the rest of a mocked harness, and require its guard call."""
    @wraps(test)
    def wrapped(self, *args, **kwargs):
        with patch("oracle_preflight.require_native_oracle") as guard:
            result = test(self, *args, **kwargs)
            guard.assert_called()
            return result
    return wrapped
