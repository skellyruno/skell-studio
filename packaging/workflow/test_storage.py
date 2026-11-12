import tempfile
import os
import unittest
from pathlib import Path
from unittest.mock import patch
import storage


class StorageTest(unittest.TestCase):
    def test_five_versions_and_four_backups_preserve_other_files(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            for i in range(1, 9):
                (root/f'20261110{i:02}').mkdir()
                (root/f'lemonstudio.before-{i}.bak').write_bytes(b'MZ')
            (root/'My song.lmn').write_text('project')
            (root/'lemonstudio.exe').write_bytes(b'MZ current')
            (root/'source').mkdir()
            cfg = {'release_roots': [str(root)], 'backup_groups': [
                {'root': str(root), 'pattern': r'lemonstudio\.before-\d\.bak'}]}
            self.assertEqual(len(storage.maintain(cfg)), 7)
            self.assertTrue((root/'2026111001').exists())
            storage.maintain(cfg, apply=True)
            self.assertEqual(len(list(root.glob('2026*'))), 5)
            self.assertEqual(len(list(root.glob('*.bak'))), 4)
            self.assertEqual((root/'My song.lmn').read_text(), 'project')
            self.assertTrue((root/'source').is_dir())
            self.assertEqual((root/'lemonstudio.exe').read_bytes(), b'MZ current')

    def test_active_old_release_is_preserved(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            for i in range(1, 7):
                (root/f'20261110{i:02}').mkdir()
            (root/'2026111001/.storage-active').touch()
            self.assertEqual(storage.plan({'release_roots': [str(root)]}), [])

    def test_four_backups_are_counted_across_install_locations(self):
        with tempfile.TemporaryDirectory() as tmp:
            roots = [Path(tmp)/'installed', Path(tmp)/'preview']
            for root in roots:
                root.mkdir()
                for i in range(3):
                    (root/f'old-{i}.bak').write_bytes(b'MZ')
            cfg = {'backup_groups': [{'locations': [
                {'root': str(root), 'pattern': r'old-\d\.bak'} for root in roots]}]}
            storage.maintain(cfg, apply=True)
            self.assertEqual(sum(len(list(root.iterdir())) for root in roots), 4)

    def test_crashed_build_marker_does_not_pin_obsolete_releases_forever(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            for i in range(1, 7):
                (root/f'20261110{i:02}').mkdir()
            (root/'2026111001/.storage-active-123').touch()
            with patch.object(storage, 'process_started', return_value=None):
                self.assertEqual(storage.plan({'release_roots': [str(root)]}), [(root/'2026111001').resolve()])
            with patch.object(storage, 'process_started', return_value=0):
                self.assertEqual(storage.plan({'release_roots': [str(root)]}), [])

    def test_only_registered_generated_cache_is_reclaimed(self):
        with tempfile.TemporaryDirectory() as tmp:
            target = Path(tmp)/'target'
            (target/'release').mkdir(parents=True)
            (target/'CACHEDIR.TAG').touch()
            (target/'release/generated').write_bytes(b'cache')
            cfg = {'cache_roots': [str(target)], 'maximum_cache_gib': 0, 'minimum_free_gib': 0}
            idle = type('Process', (), {'returncode': 0 if os.name == 'nt' else 1})()
            with patch.object(storage, 'settings', return_value=cfg), \
                 patch.object(storage.subprocess, 'run', return_value=idle):
                storage.build_preflight(target)
            self.assertFalse(target.exists())

    def test_low_disk_stops_before_cleanup(self):
        with tempfile.TemporaryDirectory() as tmp:
            with patch.object(storage, 'settings', return_value={}), \
                 patch.object(storage.shutil, 'disk_usage', return_value=type('Space', (), {'free': 10})()), \
                 patch.object(storage, 'maintain') as cleanup:
                with self.assertRaisesRegex(RuntimeError, 'No build has started'):
                    storage.build_preflight(Path(tmp))
                cleanup.assert_not_called()

    def test_registered_source_cannot_be_mistaken_for_cache(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root/'.git').mkdir()
            (root/'CACHEDIR.TAG').touch()
            (root/'source.rs').write_text('keep')
            cfg = {'cache_roots': [str(root)], 'maximum_cache_gib': 0, 'minimum_free_gib': 0}
            with patch.object(storage, 'settings', return_value=cfg):
                with self.assertRaisesRegex(RuntimeError, 'safely reclaimed'):
                    storage.build_preflight(root)
            self.assertTrue((root/'source.rs').exists())


if __name__ == '__main__':
    unittest.main()
