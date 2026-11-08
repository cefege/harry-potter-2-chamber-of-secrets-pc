"""Start subprocesses in their own process group/session."""

from __future__ import annotations

import subprocess


def popen_group(argv, **kw):
    """Launch *argv* in a new process group/session."""
    kw["start_new_session"] = True
    return subprocess.Popen(argv, **kw)
