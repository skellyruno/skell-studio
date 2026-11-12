"""Local build storage policy. Never touches installed apps, source or the shared server.

Machine paths are explicitly configured in ~/.lemon-studio-release/storage.json.
Legacy files quarantined in E:/delete are deliberately outside this policy.
"""
from pathlib import Path
from contextlib import contextmanager
import argparse
import json
import os
import re
import shutil
import subprocess
import time

CONFIG = Path.home()/'.lemon-studio-release/storage.json'
GIB = 1024**3


def settings():
    path = Path(os.environ.get('LEMON_STORAGE_CONFIG', str(CONFIG)))
    return json.loads(path.read_text(encoding='utf-8-sig')) if path.exists() else {}


def app_root():
    return Path(settings().get('app_root', str(Path.home()/'Desktop/Lemon Studio')))


def linked(p):
    return p.is_symlink() or (hasattr(p, 'is_junction') and p.is_junction())


def plain_root(p):
    p = Path(p)
    if linked(p) or p.resolve() == Path(p.anchor) or p.resolve() == Path.home().resolve():
        raise RuntimeError('Unsafe storage root: '+str(p))
    return p.resolve()


def tree_bytes(path):
    total = 0
    for base, dirs, files in os.walk(path, followlinks=False):
        dirs[:] = [d for d in dirs if not linked(Path(base)/d)]
        for name in files:
            p = Path(base)/name
            if not linked(p):
                try:
                    total += p.stat().st_size
                except FileNotFoundError:
                    pass
    return total


def process_started(pid):
    """Read process identity without sending a signal (including on Windows)."""
    if os.name == 'nt':
        import ctypes
        from ctypes import wintypes
        kernel = ctypes.WinDLL('kernel32', use_last_error=True)
        kernel.OpenProcess.restype = wintypes.HANDLE
        kernel.GetProcessTimes.argtypes = [wintypes.HANDLE] + [ctypes.POINTER(wintypes.FILETIME)]*4
        kernel.CloseHandle.argtypes = [wintypes.HANDLE]
        handle = kernel.OpenProcess(0x1000, False, pid)
        if not handle:
            if ctypes.get_last_error() == 5:
                raise OSError('Process identity unavailable')
            return None
        values = [wintypes.FILETIME() for _ in range(4)]
        try:
            if not kernel.GetProcessTimes(handle, *(ctypes.byref(v) for v in values)):
                raise OSError('Process start time unavailable')
            return ((values[0].dwHighDateTime << 32) | values[0].dwLowDateTime)/10_000_000 - 11644473600
        finally:
            kernel.CloseHandle(handle)
    result = subprocess.run(['ps', '-p', str(pid), '-o', 'lstart='],
                            capture_output=True, text=True, env=dict(os.environ, LC_ALL='C'))
    if result.returncode != 0 or not result.stdout.strip():
        return None
    return time.mktime(time.strptime(result.stdout.strip(), '%a %b %d %H:%M:%S %Y'))


def release_active(folder):
    for marker in folder.glob('.storage-active*'):
        match = re.fullmatch(r'\.storage-active-(\d+)', marker.name)
        if not match:
            return True
        try:
            started = process_started(int(match[1]))
            if started is not None and started <= marker.stat().st_mtime + 1:
                return True
        except (OSError, ValueError):
            return True  # Uncertain identity never authorizes removing an active build.
    return False


def plan(cfg):
    """Only version-numbered packages and explicitly named executable backups expire."""
    result = []
    for name in cfg.get('release_roots', []) + cfg.get('snapshot_roots', []):
        root = plain_root(name)
        if not root.exists():
            continue
        versions = sorted((p for p in root.iterdir() if p.is_dir()
                           and re.fullmatch(r'\d{10}(?:-\d{8}-\d{6})?', p.name) and not linked(p)
                           and not release_active(p)),
                          key=lambda p: p.name, reverse=True)
        for p in versions[5:]:
            result.append(p)
    for group in cfg.get('backup_groups', []):
        files = []
        for location in group.get('locations', [group]):
            root = plain_root(location['root'])
            if not root.exists():
                continue
            files.extend(p for p in root.iterdir() if p.is_file() and not linked(p)
                         and re.fullmatch(location['pattern'], p.name))
        files.sort(key=lambda p: (p.stat().st_mtime_ns, p.name), reverse=True)
        # The installed executable is build one; four rollback binaries make five.
        result.extend(files[4:])
    return result


def maintain(cfg=None, apply=False):
    cfg = settings() if cfg is None else cfg
    removed = []
    for p in plan(cfg):
        # Recheck immediately before removal, including parent replacement.
        if linked(p) or p.parent.resolve() != p.parent or linked(p.parent):
            raise RuntimeError('Storage path changed: '+str(p))
        if apply:
            if p.is_dir():
                shutil.rmtree(p)
            else:
                p.unlink()
        removed.append(str(p))
    return removed


def free_space(path, minimum_gib=20):
    probe = Path(path).absolute()
    while not probe.exists():
        probe = probe.parent
    free = shutil.disk_usage(probe).free
    if free < minimum_gib*GIB:
        raise RuntimeError(f'Build stopped: {probe.anchor} needs {minimum_gib} GiB free; '
                           f'{free/GIB:.1f} GiB available. No build has started.')


@contextmanager
def active_release(root):
    root = Path(root)
    root.mkdir(parents=True, exist_ok=True)
    marker = root/f'.storage-active-{os.getpid()}'
    marker.touch()
    try:
        yield
    finally:
        marker.unlink(missing_ok=True)
        maintain(apply=True)


def build_preflight(target, output=None):
    cfg = settings()
    target = Path(target)
    free_space(target, cfg.get('minimum_free_gib', 20))
    if output:
        free_space(output, cfg.get('minimum_free_gib', 20))
    # A compile cache is disposable; it must never consume the disk without a limit.
    # Clear oversized, explicitly registered caches before a new build, never midway.
    allowed = {plain_root(p) for p in cfg.get('cache_roots', [])}
    if target.exists() and target.resolve() in allowed:
        limit = cfg.get('maximum_cache_gib', 16)*GIB
        if tree_bytes(target) > limit:
            if linked(target) or not (target/'CACHEDIR.TAG').is_file() or (target/'.git').exists():
                raise RuntimeError('Cache cannot be safely reclaimed: '+str(target))
            if any(not re.fullmatch(r'(?:debug|release|bundled|tmp|CACHEDIR\.TAG|\.rustc_info\.json|(?:aarch64|x86_64)-[\w.-]+)', p.name)
                   for p in target.iterdir()):
                raise RuntimeError('Unrecognized files in compile cache; preserved: '+str(target))
            if os.name == 'nt':
                busy = subprocess.run(['powershell.exe', '-NoProfile', '-Command',
                    'if(Get-Process cargo,rustc -ErrorAction SilentlyContinue){exit 1}'],
                    capture_output=True).returncode != 0
            else:
                busy = subprocess.run(['pgrep', '-x', 'cargo|rustc'], capture_output=True).returncode == 0
            if busy:
                raise RuntimeError('Cache limit reached while another compiler is active; retry when it finishes.')
            shutil.rmtree(target)
    maintain(cfg, apply=True)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--apply', action='store_true')
    p.add_argument('--prepare-build', type=Path)
    args = p.parse_args()
    if args.prepare_build:
        build_preflight(args.prepare_build)
    result = {'time': time.time(), 'applied': args.apply,
              'expired': maintain(apply=args.apply)}
    if args.apply and CONFIG.exists():
        (CONFIG.parent/'storage-last-run.json').write_text(json.dumps(result, indent=2), encoding='utf-8')
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
