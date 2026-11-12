# Skell Studio — source provenance and licensing

This early engineering foundation **is a fork, not an original proprietary DAW**. It combines the two user-provided ZIP snapshots:

- `ADE-Studio-michael-lane.zip` — AQUILA / ADE's already-integrated ML Studio + newer Lemon Studio baseline.
- `ML-Studio-michael-lane.zip` — earlier ML Studio fork; nine unique files are retained under `reference/ml-only/`.

Original upstream projects and rights holders:

- https://github.com/ajdohaxhia/ADE-Studio
- https://github.com/Dorfhaus/ML-Studio
- https://github.com/ItsAlphaNeon/Lemon-Studio

The workspace declares GPL-3.0-or-later. Keep the copyright statements and the GPL license; an audit of bundled JUCE, CLAP, VST, SDK and other plugin/asset licenses is required before release. Rebranding alone **does not grant ownership of the upstream code** or permission to distribute it as closed-source proprietary software. Do not remove original author attribution.

**Do not publish binaries, installers or commercial licenses until dependencies, third-party assets, privacy and licensing have been audited.**

The current importer intentionally disables third-party automatic update calls and inherited publish workflows. Development builds should be manually validated on Windows before release.
