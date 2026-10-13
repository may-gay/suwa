#!/usr/bin/env python3
"""create original test pages in an isolated Suwayomi local-source directory."""
import argparse
import struct
import zlib
from pathlib import Path


def png(width, height, seed):
    def chunk(kind, data):
        return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data))
    rows = []
    for y in range(height):
        row = bytearray()
        for x in range(width):
            border = x % 300 < 8 or y % 420 < 8
            shade = 28 if border else 220 + (x + y + seed) % 25
            row.extend((shade, shade, shade))
        rows.append(b'\0' + row)
    return b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, 8, 2, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(b''.join(rows))) + chunk(b'IEND', b'')


parser = argparse.ArgumentParser()
parser.add_argument('local_directory', type=Path)
args = parser.parse_args()
for chapter in range(1, 4):
    folder = args.local_directory / 'Suwa Integration Fixture' / f'Chapter {chapter}'
    folder.mkdir(parents=True, exist_ok=True)
    for page in range(1, 5):
        (folder / f'{page:03}.png').write_bytes(png(600, 900 if chapter != 3 else 3600, page))
print('Created 12 test pages.')
