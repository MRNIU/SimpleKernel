import re

with open('src/filesystem/ramfs/ramfs.cpp', 'r') as f:
    content = f.read()

# Instead of regex-replacing everywhere which could be buggy, let's just add `using namespace vfs;`
# inside the `namespace ramfs {` block to make things easier, or update the types.
# Wait, `using namespace vfs;` is easiest.

content = content.replace('namespace ramfs {', 'namespace ramfs {\n\nusing namespace vfs;\n')

with open('src/filesystem/ramfs/ramfs.cpp', 'w') as f:
    f.write(content)
