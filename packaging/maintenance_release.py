"""Recognize an explicitly reserved release without a changelog entry."""
from pathlib import Path


def is_maintenance_build(root: Path, version: int) -> bool:
    marker = root / "packaging/maintenance-build.txt"
    return marker.is_file() and marker.read_text(encoding="utf-8").strip() == str(version)
