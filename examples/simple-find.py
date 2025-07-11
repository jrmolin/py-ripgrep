import os
import py_ripgrep

finder = py_ripgrep.Finder([os.path.expanduser("~/git/dotfiles"),])

found = finder.find_files()


print(f"found {len(found)} files")

if found and len(found) < 20:
    import pprint
    pprint.pprint(found)
