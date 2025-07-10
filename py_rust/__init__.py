
import os
from pathlib import Path
from typing import List

from .py_rust import Finder

def find_files(p: Path) -> List[str]:
    finder = Finder([os.path.expanduser(p)])
    return finder.find_files()

__all__ = [
    "find_files",
    "Finder",
]
