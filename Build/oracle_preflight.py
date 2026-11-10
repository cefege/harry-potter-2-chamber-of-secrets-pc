#!/usr/bin/env python3
"""Block native parity until independent oracle acceptance is integrated."""
import argparse
import sys


class OracleUnavailableError(RuntimeError):
    """Native oracle authority and provenance are not available."""


def require_native_oracle():
    """Reject all native parity; a path or digest cannot establish independence."""
    raise OracleUnavailableError(
        "native oracle parity unavailable: independent acceptance and provenance "
        "integration are required"
    )


def main():
    argparse.ArgumentParser(description=__doc__).parse_args()
    try:
        require_native_oracle()
    except OracleUnavailableError as error:
        print(f"oracle preflight: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
