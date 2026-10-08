"""Regression checks: python -m unittest discover -s tools/asset-pipe -p 'test_*.py'."""

import os
from pathlib import Path
import struct
import tempfile
import threading
from types import SimpleNamespace
import unittest
from unittest.mock import patch

from PIL import Image

import extract
from unitydb import PPtr, read_database


def database_bytes(extended=False, endian='<'):
    """Serialize a small player object, including one row in every art table."""
    raw = bytearray()

    def number(fmt, value):
        raw.extend(struct.pack(endian + fmt, value))

    def ptr(value):
        number('i', 0)
        number('q', value)

    def string(value):
        data = value.encode('utf-8')
        number('i', len(data))
        raw.extend(data)
        raw.extend(b'\0' * (-len(raw) % 4))

    ptr(1)  # m_GameObject
    raw.extend(b'\1\0\0\0')  # m_Enabled + alignment
    ptr(2)  # m_Script
    string('BandoriDatabase')
    # Serialized declaration order; inserted fields are deliberately non-null.
    texts = [10, 11, 12, 13, 14, 15, 16]
    texts += [70, 71] if extended else []
    texts += [17, 18, 19, 20]
    texts += [72] if extended else []
    for value in texts:
        ptr(value)
    number('i', 9)  # rules version
    number('i', 1)
    string('001')
    for value in [30, 31, 32, 33] + ([73] if extended else []) + [34]:
        ptr(value)
    for name, value in [('Poppin\' Party', 40), ('', 41), ('AG:カード', 42)]:
        number('i', 1)
        string(name)
        ptr(value)
    for value in [50, 51, 52, 53]:
        ptr(value)
    return bytes(raw)


class DatabaseCompatibility(unittest.TestCase):
    def test_legacy_shape_and_extended_pointer_alignment(self):
        for extended in [False, True]:
            for endian in ['<', '>']:
                with self.subTest(extended=extended, endian=endian):
                    name, db = read_database(database_bytes(extended, endian), endian == '>')
                    self.assertEqual(name, 'BandoriDatabase')
                    self.assertEqual(db.rules_version, 9)
                    self.assertEqual(db.text_assets['matchRulesJson'], PPtr(0, 20))
                    self.assertEqual(db.character_art[0]['namePlate'], PPtr(0, 34))
                    self.assertEqual(db.card_art, [{'id': 'AG:カード', 'art': PPtr(0, 42)}])
                    self.assertEqual(db.fx['tapSparkle'], PPtr(0, 53))
                    if extended:
                        self.assertEqual(db.text_assets['cardLinesJson'], PPtr(0, 70))
                        self.assertEqual(db.text_assets['skillSimpleJson'], PPtr(0, 72))
                        self.assertEqual(db.character_art[0]['sdJoy'], PPtr(0, 73))
                    else:
                        self.assertEqual(len(db.text_assets), 11)
                        self.assertEqual(set(db.character_art[0]),
                                         {'id', 'stand', 'kv', 'sd', 'sdHappy', 'namePlate'})

    def test_partial_or_trailing_objects_rejected(self):
        for extended in [False, True]:
            raw = database_bytes(extended)
            for malformed in [raw[:-1], raw + b'\0', raw[:36] + b'\xff' * 8]:
                with self.subTest(extended=extended, size=len(malformed)):
                    with self.assertRaises(ValueError):
                        read_database(malformed)


class ExtractionCompatibility(unittest.TestCase):
    def test_duplicate_image_encoded_once_while_first_write_is_pending(self):
        release = threading.Event()
        image = Image.new('RGBA', (2, 2), (10, 20, 30, 255))
        reads = []
        obj = SimpleNamespace(assets_file=SimpleNamespace(name='resources.assets'), path_id=1)

        def read():
            reads.append(1)
            return SimpleNamespace(image=image)

        obj.read = read
        save = Image.Image.save

        def delayed_save(self, *args, **kwargs):
            if not release.wait(5):
                raise RuntimeError('test encoder was never released')
            return save(self, *args, **kwargs)

        with tempfile.TemporaryDirectory() as temp, patch.object(Image.Image, 'save', delayed_save):
            pipeline = extract.Pipeline(Path(temp), {'img'}, force=True, jobs=2)
            try:
                for _ in range(2):
                    self.assertEqual(pipeline.image(obj, 'img/res/same.webp', 'img'), 'img/res/same.webp')
            finally:
                release.set()
                pipeline.drain()
                pipeline.pool.shutdown()
            self.assertEqual(reads, [1])
            self.assertEqual(pipeline.stats.errors, [])
            self.assertEqual(pipeline.stats.counts, {'img': 1})
            with Image.open(Path(temp) / 'img/res/same.webp') as result:
                self.assertEqual(result.size, (2, 2))

    def test_existing_file_skipped_unless_forced(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / 'img/existing.webp'
            path.parent.mkdir()
            path.write_bytes(b'existing')
            for force in [False, True]:
                pipeline = extract.Pipeline(Path(temp), {'img'}, force, jobs=1)
                try:
                    self.assertEqual(pipeline.want('img/existing.webp'), force)
                finally:
                    pipeline.pool.shutdown()
            self.assertEqual(path.read_bytes(), b'existing')

    def test_ffmpeg_override_is_explicit(self):
        with patch.dict(os.environ, {}, clear=True), patch('extract.shutil.which', return_value='/old/ffmpeg'):
            self.assertEqual(extract.find_ffmpeg(), '/old/ffmpeg')
            with patch.dict(os.environ, {'FFMPEG': '/chosen/ffmpeg'}):
                self.assertEqual(extract.find_ffmpeg(), '/chosen/ffmpeg')


if __name__ == '__main__':
    unittest.main()
