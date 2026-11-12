"""Build and package ML Plasma locally. Never installs or publishes anything."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import zipfile
from pathlib import PurePosixPath

ROOT = Path(__file__).resolve().parent
SOURCE = ROOT / 'vendor/gridline'


def cmake_tool():
    tool = os.environ.get('CMAKE') or shutil.which('cmake')
    if tool:
        return tool
    if os.name == 'nt':
        roots = [Path(os.environ.get('ProgramFiles', 'C:/Program Files')),
                 Path(os.environ.get('ProgramFiles(x86)', 'C:/Program Files (x86)'))]
        for root in roots:
            found = sorted(root.glob('Microsoft Visual Studio/*/*/Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin/cmake.exe'))
            if found:
                return str(found[-1])
    raise RuntimeError('CMake is required for ML Plasma')


def run(args):
    subprocess.run([str(a) for a in args], check=True)


def source_digest():
    digest = hashlib.sha256()
    source_types = {'.c', '.cpp', '.cc', '.h', '.hpp', '.inc', '.mm', '.m', '.cmake'}
    text_types = source_types | {'.svg', '.json', '.txt', '.md'}
    for path in sorted(SOURCE.rglob('*'), key=lambda p: p.relative_to(SOURCE).as_posix()):
        if path.is_file() and (path.suffix in source_types or path.name == 'CMakeLists.txt'
                               or path.relative_to(SOURCE).as_posix().startswith(('assets/fonts/', 'assets/icons/', 'assets/wordmarks/'))):
            digest.update(path.relative_to(SOURCE).as_posix().encode('utf-8') + b'\0')
            data = path.read_bytes()
            # The Windows release coordinator validates payloads built in LF checkouts.
            if path.suffix in text_types:
                data = data.replace(b'\r\n', b'\n')
            digest.update(hashlib.sha256(data).digest())
    return digest.hexdigest()


def validate_module(data, osname, arch):
    """Check native headers as well as the archive label."""
    valid = False
    if osname == 'windows' and len(data) >= 64 and data[:2] == b'MZ':
        pe = int.from_bytes(data[60:64], 'little')
        machine = int.from_bytes(data[pe + 4:pe + 6], 'little')
        valid = data[pe:pe + 4] == b'PE\0\0' and machine == {'x86_64': 0x8664, 'aarch64': 0xaa64}.get(arch)
    elif osname == 'linux' and data[:6] == b'\x7fELF\x02\x01':
        valid = int.from_bytes(data[18:20], 'little') == {'x86_64': 62, 'aarch64': 183}.get(arch)
    elif osname == 'macos':
        wanted = {'x86_64': 0x1000007, 'aarch64': 0x100000c}.get(arch)
        if data[:4] == b'\xcf\xfa\xed\xfe':
            valid = int.from_bytes(data[4:8], 'little') == wanted
        elif data[:4] == b'\xca\xfe\xba\xbe':
            count = int.from_bytes(data[4:8], 'big')
            valid = 0 < count <= 16 and any(int.from_bytes(data[8 + i * 20:12 + i * 20], 'big') == wanted for i in range(count))
    if not valid:
        raise RuntimeError('ML Plasma native binary does not match the requested OS/architecture')


def validate_bundle(path, osname, arch):
    """Cross compilation may reuse a matching native build, never a foreign engine."""
    with zipfile.ZipFile(path) as archive:
        manifest = json.loads(archive.read('engine.json'))
        if manifest.get('os') != osname or manifest.get('arch') != arch:
            raise RuntimeError('ML Plasma bundle platform does not match the Cargo target')
        if manifest.get('source_sha256') != source_digest():
            raise RuntimeError('ML Plasma native payload is stale or comes from different source')
        files = manifest.get('files', [])
        names = [entry['path'] for entry in files]
        if not names or len(set(names)) != len(names) or set(archive.namelist()) != set(names + ['engine.json']):
            raise RuntimeError('Invalid ML Plasma bundle manifest')
        for entry in files:
            name = entry['path']
            if (name != 'ML Plasma.clap' and not name.startswith(('ML Plasma.clap/', 'ML Plasma-assets/', 'Licences/'))
                    or '\\' in name or ':' in name or '..' in PurePosixPath(name).parts):
                raise RuntimeError('Invalid ML Plasma bundle path')
            if entry['size'] > 256 * 1024 * 1024:
                raise RuntimeError('ML Plasma bundle entry exceeds size limit')
            data = archive.read(name)
            if len(data) != entry['size'] or hashlib.sha256(data).hexdigest() != entry['sha256']:
                raise RuntimeError('ML Plasma bundle checksum mismatch')
        upstream = json.loads(archive.read('Licences/UPSTREAM.json'))
        if upstream['commit'] != json.loads((ROOT / 'UPSTREAM.json').read_text())['commit']:
            raise RuntimeError('ML Plasma bundle does not match the pinned source')
        module = 'ML Plasma.clap/Contents/MacOS/ML Plasma' if osname == 'macos' else 'ML Plasma.clap'
        validate_module(archive.read(module), osname, arch)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--build-dir', type=Path)
    parser.add_argument('--bundle', type=Path)
    parser.add_argument('--target-os', default=os.environ.get('CARGO_CFG_TARGET_OS', {'Windows': 'windows', 'Darwin': 'macos'}.get(platform.system(), 'linux')))
    parser.add_argument('--target-arch', default=os.environ.get('CARGO_CFG_TARGET_ARCH', {'AMD64': 'x86_64', 'arm64': 'aarch64'}.get(platform.machine(), platform.machine())))
    args = parser.parse_args()
    output = args.output.resolve()
    if args.bundle:
        validate_bundle(args.bundle, args.target_os, args.target_arch)
        output.parent.mkdir(parents=True, exist_ok=True)
        if args.bundle.resolve() != output:
            shutil.copyfile(args.bundle, output)
        print('Validated matching native ML Plasma payload:', output)
        return
    build = (args.build_dir or Path(os.environ.get('ML_PLASMA_NATIVE_DIR', output.parent / 'native'))).resolve()
    # Use the project's storage guard before every native build, including Cargo's build script.
    run([sys.executable, ROOT.parents[1] / 'packaging/workflow/storage.py', '--prepare-build', build])
    cmake = cmake_tool()
    configure = [cmake, '-S', SOURCE, '-B', build, '-DCMAKE_BUILD_TYPE=Release',
                 '-DGRIDLINE_BUNDLED_SDL2=ON', '-DGRIDLINE_COLLAB=OFF',
                 '-DGRIDLINE_GUI_TESTS=OFF', '-DGRIDLINE_DEV_ASSETS=OFF']
    if os.name == 'nt':
        # Let CMake find the installed Visual Studio version. Pinning VS 2022
        # broke Windows CI as soon as hosted runners moved to newer MSVC.
        # A configured Ninja generator does not support "-A".
        generator = os.environ.get('CMAKE_GENERATOR', '')
        if not generator or 'Visual Studio' in generator:
            configure += ['-A',
                          'ARM64' if platform.machine().lower() in ('arm64', 'aarch64') else 'x64']
    if os.environ.get('CMAKE_OSX_ARCHITECTURES'):
        configure += ['-DCMAKE_OSX_ARCHITECTURES=' + os.environ['CMAKE_OSX_ARCHITECTURES']]
    run(configure)
    run([cmake, '--build', build, '--config', 'Release', '--target', 'ml_plasma_clap', '--parallel', '6'])
    folder = build / 'plugins'
    plugin = folder / 'ML Plasma.clap'
    # Multi-config generators append the configuration to the module output directory.
    if not plugin.exists() and (folder / 'Release/ML Plasma.clap').exists():
        plugin = folder / 'Release/ML Plasma.clap'
    if not plugin.exists():
        raise RuntimeError('ML Plasma CLAP payload was not produced')
    files = []
    if plugin.is_dir():
        files.extend((p, 'ML Plasma.clap/' + p.relative_to(plugin).as_posix())
                     for p in plugin.rglob('*') if p.is_file())
    else:
        files.append((plugin, 'ML Plasma.clap'))
        # Only editor resources are needed. No startup sounds, recordings or other products' data.
        for sub in ['fonts', 'icons/ui']:
            files.extend((p, 'ML Plasma-assets/' + p.relative_to(SOURCE / 'assets').as_posix())
                         for p in (SOURCE / 'assets' / sub).rglob('*') if p.is_file())
    files += [(SOURCE / 'LICENSE', 'Licences/Gridline-GPL-3.0.txt'),
              (SOURCE / 'third_party/NOTICE.md', 'Licences/third-party-NOTICE.md'),
              (ROOT / 'UPSTREAM.json', 'Licences/UPSTREAM.json'),
              (ROOT / 'README.md', 'Licences/ML Plasma.txt')]
    for path in (SOURCE / 'third_party').rglob('*'):
        if path.is_file() and any(word in path.name.upper() for word in ['LICENSE', 'COPYING', 'COPYRIGHT']):
            files.append((path, 'Licences/third_party/' + path.relative_to(SOURCE / 'third_party').as_posix()))
    for path in (build / '_deps/sdl2-src').glob('*'):
        if path.is_file() and any(word in path.name.upper() for word in ['LICENSE', 'COPYING', 'COPYRIGHT']):
            files.append((path, 'Licences/SDL2/' + path.name))
    manifest = {'os': args.target_os, 'arch': args.target_arch, 'source_sha256': source_digest(), 'files': []}
    output.parent.mkdir(parents=True, exist_ok=True)
    temp = output.with_suffix('.zip.tmp')
    with zipfile.ZipFile(temp, 'w', zipfile.ZIP_DEFLATED, compresslevel=6) as archive:
        for path, name in sorted(files, key=lambda item: item[1]):
            data = path.read_bytes()
            manifest['files'].append({'path': name, 'size': len(data), 'sha256': hashlib.sha256(data).hexdigest()})
            archive.write(path, name)
        info = zipfile.ZipInfo('engine.json', (2026, 11, 11, 0, 0, 0))
        info.compress_type = zipfile.ZIP_DEFLATED
        archive.writestr(info, json.dumps(manifest))
    temp.replace(output)
    validate_bundle(output, args.target_os, args.target_arch)
    print('ML Plasma private payload:', output)


if __name__ == '__main__':
    main()
