#!/usr/bin/env python3
"""Static checks for the source merger baseline. Not a substitute for build/audio tests."""
from pathlib import Path
root = Path(__file__).resolve().parents[1]
required = ['Cargo.toml','Cargo.lock','LICENSE','HANDOFF.md',
    'crates/os-app/src/app.rs','crates/os-app/src/main.rs','crates/os-engine/Cargo.toml']
for rel in required:
    assert (root/rel).is_file(), f'Missing {rel}'
a = (root/'crates/os-app/src/app.rs').read_text(encoding='utf-8')
m = (root/'crates/os-app/src/main.rs').read_text(encoding='utf-8')
assert 'Updates are disabled in this Skell Studio development fork.' in a
assert '    updates::start();' not in [line for line in m.splitlines() if not line.lstrip().startswith('//')]
assert not (root/'.github/workflows/release.yml').exists()
print('Static source structure/updater checks passed (not a build or audio test).')
