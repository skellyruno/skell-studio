# ML stock plugins and ML EQ — local Development preview

The stock instrument/effect names now begin with ML: ML Sampler, ML Trio, ML Citrus, ML Chop, ML Wave, ML EQ, ML Reverb, ML Delay, ML Limiter, ML Soft Clipper, ML Fast Distortion, ML Balance, ML Stereo Enhancer, ML Transient Processor, ML Chorus, ML Stereo Shaper, ML Tape, ML Hall, ML Tesla, ML Pulp Filter and ML Press. Their own editor wordmarks and the plugin picker previews match. The bundled ML Master Assistant also uses ML Studio in its native editor, VST3 product and vendor metadata, and picker preview. Its VST3 class IDs are preserved.

Plugin IDs, parameter IDs, state encodings and FL-compatible file identifiers stay stable. Old display names resolve to the renamed built-in plugins. Old preset headers/factory directories remain readable. Presets in coexisting legacy/current user folders remain accessible without overwriting each other. Existing user-assigned channel, slot and preset names are preserved.

## ML EQ

- Blue response curve and controls, neutral gray surfaces, natural-width lettering and a larger initial window.
- Numbered graph nodes match a seven-band overview. Click a band's heading to select it; Ctrl-click selects several. Right-click opens its band menu.
- Frequency and gain readouts support dragging, wheel adjustment and double-click numeric entry. Existing host automation menus remain available.
- Power switches mute bands; a deleted band can be restored from its overview power switch. These edits support Undo/Redo.
- Reset applies to selected bands. Invert gain changes selected boosts to cuts and vice versa. Auto fit adjusts the graph's vertical range without changing sound.
- Existing dynamic EQ, spectral dynamics, de-harsh, audition, A/B, gain match and analyzer controls remain available. Band controls opens the detailed drawer.
- Settings > Visibility > ML EQ can hide the new overview/quick actions or the floating band card. Hiding them preserves settings and processing.

This update changes the editing experience, not the EQ DSP or its automation/state schema. Windows Development only; no public release. Mac/Linux and hands-on audio/visual testing are outstanding.
