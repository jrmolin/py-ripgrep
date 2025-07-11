import os
import py_ripgrep

finder = py_ripgrep.Finder([os.path.expanduser("~/git/dotfiles"),])

# look for files with XXX or TODO
finder.add_regex("XXX|TODO")
found = finder.search()

print(f"found {len(found)} files")

for k, v in found.items():
    print(f"{k}:{len(v)}")
    for f in v:
        print(f"{f.line_number} - {f.match}")

if found and len(found) < 20:
    import pprint
    pprint.pprint(found)
