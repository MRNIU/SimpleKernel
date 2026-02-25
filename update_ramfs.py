import re

with open('src/filesystem/ramfs/include/ramfs.hpp', 'r') as f:
    content = f.read()

# Replace types with vfs::
types = ['FileSystem', 'Inode', 'BlockDevice', 'FileType', 'FileOps', 'File', 'DirEntry', 'SeekWhence']
for t in types:
    content = re.sub(r'(?<!vfs::)\b' + t + r'\b', 'vfs::' + t, content)

with open('src/filesystem/ramfs/include/ramfs.hpp', 'w') as f:
    f.write(content)
