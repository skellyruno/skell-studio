# ML Studio User Guide

## Getting started
### Make your first song
beat beginner first project getting started
Guide: rack-steps
1. Use File > New to start with your default template. Set the template in General settings and choose an output device in Options > Audio settings.
2. Add an instrument with the Add menu or the plugin picker. Open the Channel Rack and click steps on a drum channel to make a beat.
3. Right-click an instrument's row and open Piano Roll to draw notes. Play in Pattern mode to hear the current pattern.
4. Open the Playlist, choose the pattern and click a track to place it. Switch to Song mode to hear the arrangement.
5. Save the project, then use File > Export to create an audio file.

### Understand the main windows
workspace browser mixer channel rack piano roll playlist arrangement
The Channel Rack holds instruments, samples and step patterns. Piano Roll edits notes for one channel in the current pattern. The Playlist arranges patterns, recordings and automation over time.
The Mixer controls audio level, pan, effects and routing. The Browser finds samples, projects and presets. Use the toolbar shortcuts or the View menu to show these windows.

### Set tempo and play the right thing
bpm metronome transport playback loop song pattern space
Guide: transport
Choose PAT to hear the current pattern or SONG to hear the Playlist. Press Space to start or stop, or use the toolbar transport.
Adjust BPM in the toolbar. Enable the metronome to hear the beat and click in the Playlist ruler to set the playhead.

### Save, reopen and recover a project
lmn flp backup save as autosave portable missing files
Use File > Save to keep working or Save As to make a separate version. Keep the project's audio files and installed plugins available when reopening it.
Use the File menu's ZIP project export when you need a portable project and its audio. Third-party plugins must be installed on the destination machine.
If a project crashes or a save is lost, check the available backups before overwriting the original project.

### Import a project from FL Studio
flp compatibility import report missing sample plugin migrate
Open the FLP through File > Open. Review the Project Import Report if it lists missing files or unresolved links, and point the project to the original samples where needed.
Install compatible plugin formats before opening a project that uses third-party plugins. Check playback, automation, routing and settings against the original, then save a separate working copy.

### Undo and redo
history revert mistake ctrl z
Use Edit > Undo or Undo history to step back through project edits. Redo restores edits that were undone if you have not made a new conflicting edit.
Editors with their own Undo, such as the timing editor, may operate on their draft. Applying an edit can create a separate project undo step.

## Audio and recording
### Set up an audio device
sound output speakers headphones interface asio wasapi coreaudio buffer latency
Guide: audio-device
Open Options > Audio settings. Choose an output device and verify that the device's sample rate and channels match your interface settings.
Start with a buffer that plays reliably. Lower it if monitoring feels delayed, or raise it if playback crackles or the underrun count rises. On Windows, select your interface's ASIO driver if available.

### Record a microphone or instrument
record vocals guitar interface input arm monitor take
Guide: record
1. Select an empty Mixer insert and choose your interface input in its input selector.
2. Check that the meter moves and set a safe input level on the interface. Arm disk recording on the insert.
3. Enable recording and choose audio recording if prompted. Start playback, perform the take, then stop and check the recorded clip.
Use headphones when monitoring a microphone. If the sound is doubled, check whether both interface direct monitoring and software monitoring are active.

### Prevent feedback and choose a recording input
record dry mono stereo left right input monitoring bleed
Guide: mixer-input
Use a mono input for a single microphone or instrument, and a stereo pair for a stereo source. Verify the input selector on the Mixer track before arming it.
Listen on headphones and disable any unintended second monitoring path. Record a short test, then play it back to check the level, channel balance and timing.

### Record notes from a MIDI keyboard
midi controller record piano keyboard velocity
Guide: midi
Enable the device in Options > MIDI settings. Select the instrument channel, test a few notes, then enable recording and select note recording if prompted.
Start playback in the desired mode and perform. Open the pattern's Piano Roll to correct timing and velocity.

### Fix delay, crackles or no sound
latency underruns cpu stutter dropout silence device troubleshooting
Guide: audio-buffer
If nothing plays, check the output device, master level, muted tracks, song/pattern mode and routing to Master. If only one channel is silent, check its sample or plugin and its Mixer destination.
For dropouts, raise the buffer and temporarily bypass expensive effects to isolate the cause. Freeze or bounce finished parts if needed.
Plugin delay compensation aligns paths using delays reported by plugins. Input monitoring still has hardware and processing latency, so use a small stable buffer or direct monitoring when recording.

### Set up a keyboard or mixer controller
MIDI controller Komplete Kontrol KeyLab Arturia Behringer X-Touch nanoKONTROL Akai Novation Launchkey M-Audio Nektar Mackie MCU
Guide: midi
Open Settings > MIDI, select an input device, enable it and choose its Controller type. This choice is saved separately for each device in your profile.
For musical keys and pads, choose Generic MIDI / MIDI Learn. Use the input role to separate an instrument port from a dedicated DAW controls port. Use Instruments + controls when a keyboard has one shared port. Right-click a software knob, choose Link to controller, then move the hardware control. For an endless encoder, choose the Relative mode that matches its MIDI format.
Under DAW MIDI Learn, select an action and press a hardware button on the selected input. Single press mode suits CC buttons that send no release. Save Learn preset exports the actions and button behavior to a file. Load Learn preset applies that file to the selected input and replaces its learned actions.
For a controller with Mackie Control Universal support, set its hardware mode to MCU, select its dedicated DAW input in ML Studio, and choose Mackie Control or the matching named MCU profile. Profiles include Kontrol, KeyLab, X-Touch, Oxygen Pro, nanoKONTROL Studio, SSL UF1/UF8 and iCON QCon. Use the hardware's matching Mackie mode. Select the dedicated DAW output in the output list, then click Pair selected output. Hardware with compatible motor faders, button LEDs, pan rings or track-name displays receives feedback over that output.
For Launchkey MK4, select its native profile on the DAW input and pair the DAW output to enter DAW mode. Its DAW pads select and mute Mixer inserts, and its mixer encoders and available faders control pan and level. Keep the instrument port available for playing keys. The nanoKONTROL2 factory profile uses CC mode. For Akai and other controllers configured to transmit MIDI Machine Control, choose the MMC profile to receive transport commands alongside notes.
If notes work but transport does not, turn on the MIDI Monitor and check whether pressing a button generates a message on the selected DAW port. If an input shows in use, close the other application using that MIDI port. Use Refresh device list after connecting hardware.
Hide the setup controls under Settings > General > Workspace Visibility > Input customization > MIDI controller setup. Saved modes and mappings continue to operate when the controls are hidden.

### Use MIDI, typing keyboard and step entry
step edit typing keyboard controllers performance
Guide: typing
Enable Typing keyboard to piano in the toolbar to play from computer keys. Turn it off if those keys should trigger editing shortcuts.
Use Step Edit with playback stopped to enter notes one step at a time. Check the selected channel, current position and snap before entering notes.
Use MIDI settings to configure enabled devices and controller inputs. Save a small test project before setting up a complex controller mapping.

### Enter notes one step at a time
step edit step recording midi chord input
Guide: step
Stop playback and enable Step Edit in the toolbar. Select an instrument and set the playhead and snap.
Play a note or chord from a MIDI or typing keyboard. Each entry advances by one snap step. Turn Step Edit off before returning to live playing.

## Channel Rack and Piano Roll
### Add, replace and route an instrument
channel rack generator vst synth sampler insert instrument
Guide: rack-route
Use Add or the plugin picker to create an instrument channel. To insert another instrument from the rack, right-click a row and choose Insert instrument.
Use the rack channel's Mixer track assignment to send its sound to an insert. Name the channel and insert so they are easy to find again.

### Program steps and patterns
drum sequencer swing kick snare hi hat pattern
Guide: rack-steps
Choose or create a pattern. Click a step to turn it on or off, and play in PAT mode to audition it.
Keep variations in separate patterns so they can be arranged independently. Adjust swing to change the feel and use per-channel loop lengths where available.

### Draw, move and edit notes
piano roll pencil brush select resize velocity note properties
Guide: piano-notes
Open a channel's Piano Roll. Use Draw to add notes, drag a note to change time or pitch, and drag its edge to resize it.
Use Select to work on several notes. Right-click empty space with Draw or Brush to clear selection. Use the lower lane to edit velocity or other note values.
Double-click a note to inspect its properties. Set snap according to the grid you want before moving or resizing notes.

### Use Glide Notes and slide pitches
glide slide portamento normal note type
Guide: piano-notes
Choose Glide Note in note properties to place a glide. Select an existing normal or glide note to pick up that placement type.
Place a glide against a sounding note on a compatible instrument and adjust its length to control the transition. Check the result on the instrument you are using, since third-party plugins can handle note slides differently.

### Zoom, navigate and place patterns
piano roll alt wheel vertical zoom place pattern playlist toolbar
Guide: place-pattern
Use Alt + wheel over Piano Roll to resize note rows vertically. Use the horizontal overview to move through the pattern and the vertical controls to bring the notes into view.
Use Place Pattern to Playlist in the Piano Roll toolbar to put the current pattern at the Playlist playhead. It finds the first track with space for the clip, opens the Playlist and highlights the result.

### Quantize and shape notes
quantize timing strum arpeggio flam chop glue legato flip randomize
Guide: piano-tools
Select the notes to process, then open the relevant command in the Piano Roll Tools menu. Quantize adjusts timing toward a grid. Strum offsets chord notes, and Arpeggiate turns chord notes into a repeating sequence.
Use Chop to split notes, Glue to join suitable notes and Legato to extend notes toward the following ones. Preview the tool, apply it, or cancel to keep the original.

### Edit per-note expression with MPE
mpe pitch bend pressure slide timbre controller lanes
Guide: piano-mpe
Open MPE / Note expression in the Piano Roll menu. Enable it for the instrument and show the expression lanes. Use a compatible instrument or patch and match its pitch bend range.
Select one note to edit its expression curves. For a hardware controller, also match its zone and member channels to the instrument settings.

### Work with chord and scale tools
chord workspace harmony key scale scale highlighting chord progressions
Guide: chords
Open Chord Workspace from the Piano Roll shortcut where shown. Choose a scale or chord and audition it in the current instrument before placing notes.
Use the Piano Roll's scale guides as a reference. If the shortcut is hidden, look under Settings > Visibility > Piano Roll.

### Resize lanes and use the pattern overview
piano roll minimap scroll zoom velocity lane splitter
Guide: piano-overview
Drag the splitter above the lower event lane to give it more space. Double-click the splitter to hide or show the lane.
Use the overview scrollbar to move through a long pattern. Zoom in for small edits and out to check the whole phrase.

### Edit velocity and note expression
velocity pan release note properties ghost notes scale key event
Guide: piano-expression
Use the Piano Roll lower lane to change velocity and other available note properties. Select notes first when you only want to change part of a phrase.
Use ghost notes for reference to other channels and key or scale guides to see usable pitches. Drag the lane splitter to make the editing area taller.

## Playlist and audio editing
### Build an arrangement
playlist clips tracks markers patterns drag arrangement
Guide: clip-picker
Choose a pattern, audio clip or automation source, then place it on the Playlist. Drag a clip to move it and use its edges to trim it. Turn on snap for grid-aligned placement.
Use the Playlist tools to draw, paint, split, mute or select clips. Right-click the ruler for marker commands and use the Arrangement selector to manage versions of the song.

### Use the clips picker and loop previews
clip picker pattern picker preview audition loop stop playlist left panel
Guide: clip-picker
In the Playlist menu, enable Show clip picker panel. Its tabs separate patterns, audio and automation. Click a row to choose what to place.
Click the Play icon on the right to loop an audio clip or supported pattern. Click Stop on the same row or start another preview. For a pattern that relies on a hosted third-party instrument, render it to audio before auditioning it here.
Show or hide the preview buttons in Settings > Visibility > Playlist.

### Import, trim and fade audio
sample drag wav mp3 audio clip fades crop stretch
Guide: stretch
Drag an audio file from the Browser or your file manager into the Playlist. Use the clip edges to trim, the fade handles to ease in or out, and the clip menu for source commands.
Use Stretch mode when dragging an edge should change the clip's timing. Turn it off to return to trimming. Use the audio channel's settings for pitch or stretching method.

### Make a clip independent
make unique clone duplicate shared audio channel route
Use Make unique in the clip menu before changing a source that several clips share. Edits to a shared source can affect other instances.
After making it unique, rename the new source and make the change there. Use a unique audio channel when two instances need separate Mixer destinations.

### Set up an alternate arrangement
arrangement duplicate playlist version marker sections
Guide: arrangement
Use the Arrangement selector to manage alternate versions of the Playlist. Keep a separate arrangement for an alternate edit or song structure.
Review which arrangement is active before recording, bouncing or exporting. Patterns and channels can still be shared between arrangements.

### Edit audio timing with warp markers
warp transient flex time guitar rhythm audio timing detect quantize snap
Guide: warp
1. Click the timing-pin button on the Playlist track, or open Edit audio timing from an audio clip's menu. The editor detects transients in the clip.
2. Click a detected line to create a timing marker. Drag anywhere along a movable marker line to adjust timing. Use Snap to follow the selected grid.
3. Choose Select and drag across several points. Shift-drag adds more. Use Quantize to move the selection toward the grid, and choose Grid and Strength before quantizing.
4. Use Ctrl + wheel to zoom time, Alt + wheel to scale the waveform display, and Fit to see the whole clip. Click Apply to render the change. Close to discard the draft or use Undo to reverse a change.

### Align multiple performances
vocal align vocalign dub guide timing takes imported vocals instruments
Select at least two audio clips, including the guide, then right-click the intended guide and choose Vocal Align to this clip.
Click Analyze and compare the guide, original and aligned views. Adjust Tightness and listen to the preview before applying. Imported clips work as well as recorded takes. Matching phrasing and comparable source sections give better results.
Use a guide that already has the timing you want. Check sustained, quiet or poorly matched sections by ear.

### Tune vocals in the Playlist
pitch correction vocal tuning melody notes formant tune
Guide: vocal
Click the microphone icon on the track with the vocal clip to expand the tuning editor. Let it detect notes, then use the editing tools to select notes and adjust pitch or timing where supported.
Compare corrections with the original and use Undo if you move the wrong note. Closing the editor keeps existing corrections. Workspace Visibility can hide its button without removing tuning.

### Comp and organize takes
take lanes comping record vocals alternatives
Guide: comp
Right-click the Playlist track and choose Create take lanes / comp when available. Keep alternate performances on lanes under the same parent.
Use the comp tool to choose sections for the main take. Audition transitions and adjust boundaries or fades before bouncing a final take.

### Freeze or bounce a track
render new track replace cpu bounce audio freeze unfreeze
Guide: track-menu
Right-click a Playlist track header. Freeze track renders the track to save CPU and allows you to return to the editable sources with Unfreeze.
Use Bounce audio > Bounce audio in new track to keep the source on its own track, or Bounce and replace track to work with the rendered clip in place. Save first and compare the result, especially when effects and routing are important.

### Arrange with groups, markers and zoom
playlist overview minimap group clips ruler zoom scroll
Guide: playlist-overview
Use the overview above the Playlist to navigate. Drag the thumb to move the view and drag its ends to change the horizontal range.
Group related clips when they should move together. Use time markers to label sections and, where applicable, set start, loop or skip behavior. Use the Playlist snap choice before moving an entire section.

## Mixer, routing and effects
### Set levels, pan and routing
mixer volume pan master signal flow insert route
Guide: mixer-level
Route a Channel Rack instrument or audio clip to a named Mixer insert. Use its fader and pan controls to balance it, and check its meter.
Effects in the insert's slots run in order. Check that the track reaches Master through a direct route or a bus. Avoid clipping the Master if you are exporting a mix.

### Route audio with the miniature Mixer picker
route picker send to mixer track track number audio clip channel rack
Guide: rack-route
Click the Channel Rack's Mixer track number or the channel window's TRACK box to open the miniature Mixer picker. For a Playlist audio clip, use Send to mixer track in its menu.
Search by insert name or number and click a strip to assign the route. Press Escape to cancel. Clips sharing the same source also share its route, so use Make unique first when they need separate assignments.

### Create sends, buses and sidechains
send triangle reverb aux bus route sidechain compressor ducking
Guide: mixer-send
Select the source Mixer track. Click the routing triangle beneath the destination track to create a send, or right-click it for routing choices. Use the send level to control the amount.
For parallel effects, keep the dry route and send some signal to a reverb or delay return. For a bus, use Route to this track only if the bus should carry the full signal.
For sidechain ducking, create a sidechain route and choose that source in the effect slot's Sidechain input menu. Check the plugin's own sidechain controls.

### Choose the Mixer layout and built-in strip
full mixer custom strip traditional gate tame comp drive low cut high mid low
Guide: mixer-layout
Use the Mixer view buttons to choose Full or Traditional. In Full Mixer, right-click an insert and choose Channel layout > Custom strip to see its built-in EQ, Low Cut, Gate, Drive, Comp and Tame. Vox adds vocal controls.
Standard (FL-style) emphasizes the meter. The overall Mixer view and each insert's channel layout are separate choices.

### Lock a Mixer channel
lock padlock fader protect routing effects
Guide: mixer-lock
Click the padlock on a Mixer channel to protect its configuration from accidental manual changes. Click it again when you want to edit the channel.
Playback and existing automation continue while it is locked. If the icon is missing, look under Settings > Visibility > Mixer.

### Add, reorder and bypass effects
plugin picker effects slot vst eq insert replace bypass
Guide: effect-slot
Click an empty effect slot and choose an effect. Click a loaded slot to open its controls. Use the slot menu to replace, move, copy or remove an effect where offered.
Use the slot enable switch to compare the effect with the dry signal, and check the channel level after changes that add gain.

### Save and reuse effect chains
effect chains plugin picker save load favorites import export
Guide: effect-chain
Open the effect picker and choose Effect Chains. Right-click a Mixer insert and choose Save effect chain to capture its ordered effects and settings. To build one in the picker, Ctrl-click the effects you want and choose Save as chain.
Click a chain to select it, Ctrl/Cmd-click to toggle individual chains, or Shift-click to select a range. Click Choose to load them onto the selected Mixer insert, or drag a selected chain to the Mixer insert you want. Dragging a selected card carries the whole selection. Chains load in the order shown, with their effects in order, into consecutive free slots. One Undo removes the whole insertion.
Use the available rename, duplicate, favorite, import and export controls to organize and share chains. Recipients need the plugins used by the chain.

### Mix with Lemon Parametric EQ
eq bell shelf low pass high pass dynamic spectral tame deharsh listen solo
Guide: effect-slot
Add Lemon Parametric EQ to an effect slot. Select a band and adjust frequency, gain and Q. Use the band type controls for a bell, shelf or cut.
Use the band card to bypass or listen to an individual band. Open the band's Dynamic or Spectral controls for level-dependent changes and audition before committing to deep cuts.
The Mixer mini EQ and an EQ plugin in a slot are separate processors. Check which one is enabled when troubleshooting a tonal change.

### Use dynamics, saturation and time effects
limiter compressor noise gate clipping reverb delay drive transient chorus
Guide: effect-slot
Use a compressor to reduce level changes, a gate to control quiet sections, and a limiter or clipper to control peaks. Match the output level before comparing an effect with bypass.
Use drive and saturation for harmonics, Transient Processor to shape attack and sustain, and chorus or stereo effects to adjust width. Put reverb or delay on a send when several tracks should share it.

### Correct headphone monitoring
headphones phones autoeq monitor correction profile
Guide: phones
Click the Master strip's headphone icon, search for the exact model, and choose the matching profile. Once downloaded, the profile can be used offline.
Use the PHONES switch to compare or bypass correction. This processing applies to the listening output and is excluded from audio exports, bounce and recording.

### Master a mix with Master Assistant
master assistant loudness reference headphone correction
Guide: master-assistant
Start with a balanced mix and leave room at the Master output. Open Master Assistant from the Master strip and let it analyze representative playback before comparing its settings.
Set a target that fits the delivery format and check that the result is not clipping. Use Detailed controls to inspect or adjust EQ, multiband compression, stereo width and limiting. Compare at similar loudness before exporting.

## Plugins, instruments and presets
### Find installed plugins
plugin scan vst vst3 vst2 clap plugin manager add folders missing plugin
Guide: plugin-scan
Open Settings > Plugins. Add the folders where your plugins are installed, then click Find new plugins. Use Rescan all when the inventory needs rebuilding.
Use a plugin format and architecture that match your system. After a scan, find instruments in Add or the rack picker, and effects in a Mixer slot's picker.

### Search, favorite and manage the plugin picker
f8 plugin picker favorites blacklist installed filter
Open the picker from the rack or an effect slot to search for the right type of plugin. F8 opens the general picker. Check the target before choosing, since an effect goes to a Mixer insert and an instrument goes to the rack.
Use the installed and favorites filters to narrow the list. If a plugin is missing, check its blacklist status and scan results in Settings > Plugins.

### Recover a missing plugin or preset
missing plugin wrong format vst3 clap preset blacklisted
Guide: plugin-scan
Check the plugin name and format the project expects. Install that version when available, add its folder to the plugin manager, and run a scan.
If an effect slot opens with a missing plugin, preserve a copy of the project before replacing the slot. Compare the replacement's sound and reconnect automation where needed.

### Load and save presets
preset vstpreset fxp fxb ospreset patch bank browse previous next
Use the arrows or preset menu in the plugin wrapper's title bar to step through presets. The menu offers browsing and saving your own presets where supported.
Drop a compatible preset onto the intended plugin window or slot to load it there. Use Save preset as to preserve an edited version before switching to another preset.

### Play and shape samples in Lemon Sampler
sample sampler root note loop start end envelopes reverse pitch
Load a sample into a sampler channel. Set the start and end, pitch or root note, and loop behavior to suit the sound. Open Piano Roll and play notes at different pitches to check it.
Use the channel's envelopes and filters to shape playback. Keep long files on disk when supported if you need to reduce memory use.

### Design sounds with Lemon Wave
synth oscillator wavetable modulation lfo envelope unassign
Add Lemon Wave as an instrument and choose a preset as a starting point. Change oscillator and filter settings while playing a few notes, then shape the sound with the envelope.
Assign an LFO or envelope to a control to add movement. Right-click a modulated control to manage or remove its assignment. Save a new preset before changing patches.

### Use Lemon Trio and Citrus
trio synthesizer oscillator citrus sytrus fm presets
Use Lemon Trio when you want to layer three oscillators. Start with one oscillator, then add detuned or octave layers and adjust the filter and envelope.
In Citrus, start from a preset and change operators or their routing in small steps. Save a copy before making large patch changes.

### Slice audio with Lemon Chop
chop slices sample beat slicer pads
Load an audio sample into Lemon Chop and check its detected slices. Adjust slice boundaries before playing them from pads or MIDI notes.
Use the slices to build a pattern in Piano Roll. Compare playback with the original loop to check the trigger positions and tempo.

### Use an ARA audio editor
ara melodyne plugin audio edit compatibility
Install a compatible ARA editor for your platform. Open it through the audio clip's ARA action where available and let it read the clip.
Complete pitch or timing edits in the editor, then save the project. Some clip operations, including inline warp, require rendering the ARA edit first.

## Automation and control
### Create an automation clip
automation parameter ramp volume filter right click last tweaked
Right-click a supported knob or plugin parameter and choose Create automation clip. For a plugin parameter that does not expose the command directly, use Tools > Last tweaked or Browse parameters to find it.
Place the automation clip in the Playlist and edit its points. Play the affected section and watch the target control to confirm the link.

### Edit curves and manage targets
automation points tension curve link move channel track
Add points on the automation line and drag them to change value and time. Use point menus or curve controls for the shape between points.
Moving the clip to another Playlist row changes where it is displayed. Check its parameter link when you want to automate a different control. Use a separate clip or link for an independent target.

### Map a MIDI knob or controller
link to controller midi learn multilink hardware
Guide: midi
Enable your controller in MIDI settings. Right-click a supported parameter and choose Link to controller, then move the hardware control when prompted.
Test the full range and check whether the link should apply to the current project or to a reusable control setup.

### Customize mouse and keyboard behavior
controls input bindings modifier shortcut marquee right drag wheel
Guide: controls
Open Settings > Controls. Choose an editor context, event and modifiers, then search for an action. Use Capture to record a key and try it in that editor.
A more specific context can override a broader one. Use Restore event to return to inherited behavior, or Do nothing to suppress the gesture.
Clicks and drags can have separate assignments. Set a left click to insert a note and left drag to select a region where those actions are available.

### Restore a shortcut that stopped working
shortcut override keyboard binding context hotkey typing keyboard
Guide: controls
Check which editor is active and whether a text field or dialog is taking keys. Turn off Typing keyboard to piano if a note key should be used for editing.
Open Settings > Controls and look for a matching event in the editor and its parent contexts. Use Restore event to remove an override and return to inherited behavior.

### Build action sequences and toolbar buttons
macro script lsa saved actions custom toolbar import export
Guide: controls
In Settings > Controls, use Append to build a sequence of compatible editing actions. Save a name to reuse it and choose Pin as toolbar button to make it accessible from the toolbar.
Use Export to back up controls. Import accepts a controls JSON file or a portable .lsa action script. Portable scripts run supported ML Studio actions and do not execute system commands.

## Themes, workspace and sharing
### Show or hide interface features
workspace visibility remove button toolbar settings tabs clip picker
Guide: visibility
Open Settings > General > Workspace Visibility, or choose the Visibility tab. Expand a section or search for the control, then toggle it.
These choices save to your profile and follow you between projects. Use the Playlist menu's Show clip picker panel switch to open or close the picker itself.
Restore default visibility brings back the default show/hide choices. Existing toolbar layout settings still apply.

### Customize menus
menu layout arrange submenu hide items piano roll channel rack mixer
Guide: menus
Open Settings > Menus and choose the menu to customize. Show or hide entries and use the structure controls to reorder them or place them in submenus.
Menu layouts are saved to your profile. Restore the menu's defaults if a command becomes hard to find.

### Show a window or restore your workspace
window missing channel rack mixer browser playlist layout
Use View or the toolbar to reopen the missing window. If its shortcut is hidden, use Settings > Visibility to show it.
A project can restore saved window positions. If a window is behind another one, bring it forward with View and move or resize it before saving the layout.

### Change theme, icons and cursors
theme vintage fl 2005 theme editor cursor pack toolbar colors
Guide: theme
Open Settings > Theme and choose a skin or theme. Use the theme controls to adjust colors, icons, cursor packs and Mixer appearance where available.
Use the Theme Editor to preview changes and apply or cancel them. Export the theme if you want to share or reuse it on another installation.

### Browse samples and release history
browser folders sample preview presets plugin downloads changelog
Guide: browser
Use the Browser tabs to find samples, presets, projects and plugin downloads. Add your own folders in Settings > File to keep libraries easy to find.
Use the sample preview controls to audition files before placing them. Open Help > Change Log to browse or search the bundled release history offline.

### Export a finished song, stems or MIDI
export render wav mp3 stems multitrack midi
Guide: export
Choose Song mode and confirm the arrangement and time range. Use File > Export and choose a format, destination and quality settings.
Use WAV for a full-quality mix, MP3 when a smaller listening copy is useful, and MIDI for notes and timing. Use split Mixer tracks when you need separate audio stems.
Listen to the exported file and verify its start, end, levels and effects before sending it.

### Send a project with its audio files
share project pack zipped audio samples collaborate
Guide: export
Use File's ZIP project export to gather the project and its referenced audio files. Open a copy of the archive to check that it contains what the recipient needs.
Tell the recipient which third-party plugins and formats the project uses. For a collaborator without those plugins, include audio renders of important tracks.

### Report a problem and check for updates
report bug build update restart troubleshooting support
Use Report a Bug and include your build, operating system, exact steps, expected result and actual result. A small project or screenshot helps when it can be shared safely.
Use the update control to check for a signed release. Save your work before restarting to apply it. You can also install a public update from the official downloads page.

### Discover free plugins
plugin downloads favorites freeware open source portable compatibility installation

Open Browser > Plugin Downloads. Show opens filters for Favorites, Native download, Portable install, Open source and Freeware. Sort switches between categories and alphabetical names. Search matches every word across names, descriptions and licences. Reset filters and search returns to the full catalog.

Click the star beside a plugin to save it to your profile's download favorites. These are separate from favorites in the installed Plugin Picker. Download cards distinguish portable packages, installers/archives and website-only entries. Get from developer opens the official website; some free plugins require a vendor account or manager. Complete the vendor's installation, then click Scan Plugins. A downloaded installer is not proof the plugin is installed.

Direct downloads are checked against the catalog checksum before opening. Cancel stops the current download. Downloads opens the package folder. Hide the discovery controls under Settings > Visibility > Browser > Plugin Downloads filters and favorites. Hiding them shows the unfiltered category browser and preserves your saved favorites and choices. Hiding the whole tab does not remove any installed plugins.

### Tesla time stretch and pitch shift
Tesla audio editor duration pitch semitones Signalsmith stretch

In Tesla, select the audio to change, or clear the selection to process the whole sample. Open Tools > Time stretch / pitch shift, or click the stretch tool. Duration changes the length independently of pitch: 200% is twice as long; 50% is half as long. Pitch uses semitones: +12 is one octave up. Click either value to type an exact number, or use its minus and plus buttons. Duration supports 25–400%; pitch supports -24 to +24 semitones.

Apply renders in the background and shows Processing. Cancel or Escape keeps the original audio. Wait for a cancelled render to finish before opening another. On success the processed range is selected; Undo restores the previous audio and markers in one step. Source files on disk are never overwritten. Save As or send the edited audio to the Playlist when ready. Markers, regions and loops follow the time change; whole-sample changes also adjust stored sample tempo. The tool supports samples and results up to 20 minutes. Stop recording before processing.

Hide its toolbar button and Tools menu entry in Settings > Visibility > Toolbar extras > Tesla time stretch and pitch shift. Existing audio and applied edits are preserved.

### Lemon Tape: tape color and saturation
Lemon Tape Airwindows ToTape6 tape saturation soften head bump flutter

Add Lemon Tape in a Mixer effect slot. Input drives the tape model (-12 to +12 dB); Soften rounds the high frequencies; Head bump adds low-frequency resonance; Flutter introduces pitch movement. Output trims the processed level and Mix blends dry and processed audio. Start with Gentle Warmth or Soft Mix Bus. Match output loudness when judging the effect. Drum Color is more driven; Worn Cassette adds obvious softness and movement. The reel artwork illustrates the effect, not an audio meter.

Knobs support typing values, reset, automation and the usual plugin preset saving. Existing effects and old projects keep their original sound. Lemon Tape uses Chris Johnson's MIT-licensed Airwindows ToTape6 audio processor with Lemon controls; credit and licence are included. This is a creative tape model, not a claim to reproduce one specific physical machine. At very high sample rates, flutter depth is bounded to fit the processor's delay line. Below 22.05 kHz, internal coefficients use a 22.05 kHz floor for stability.

Hide it from new-plugin menus with Settings > Visibility > Toolbar extras > Lemon Tape in plugin menus. Existing instances still play, save and reopen. Plugin Manager's existing Show control also remains available.


### Lemon Hall reverb
Keywords: hall, reverb, Dragonfly, Freeverb3, early reflections, damping, pre-delay

Add **Lemon Hall** in a Mixer effect slot for a stereo hall with early reflections and a modulated reverb tail. It is an additional effect; existing Lemon Reverb instances keep their original sound. Try Vocal Hall or Short Drum Space first. On a dedicated send, use Hall Send - Fully Wet; adjust the channel's send amount to blend the effect.

Decay sets the tail length (0.20–12 seconds). Pre-delay separates the dry attack from the whole wet signal (0–250 ms). Early Balance blends early reflections at 100% with the diffuse late tail at 0%; start around 25%. Diffusion softens the tail's texture. Low Cut and High Cut filter the wet signal, leaving the dry signal intact. HF Decay sets how quickly high frequencies fade relative to the main decay: lower values give a darker tail. Modulation adds movement, Width sets stereo spread (0% is mono), and Mix blends dry and wet. Changes are smoothed; moving Pre-delay can briefly bend the wet pitch, so stop or use gentle changes if unwanted.

Drag a knob, use its wheel, or right-click for the standard value, reset and automation options. Save your settings in a plugin preset or project. The display is a settings illustration, not a live audio meter. Hall geometry and modulation speed are fixed; this is a focused Lemon interface to the Dragonfly/Freeverb3 hall engine, not every control from the original plugin.

To hide it from new plugin menus, turn off **Lemon Hall in plugin menus** in Settings > General > Workspace Visibility > Choose what to show > Toolbar extras. Saved instances still load and play. This option is saved with your profile.


### Clean up audio in Tesla
Keywords: cleanup, clean up, denoise, noise reduction, noise profile, hiss, hum, Tesla, apply to track

Right-click an audio clip in the Playlist and choose **Audio processing > Clean up audio in Tesla**. Tesla opens a copy of the clip's current source audio. It includes the full source file, so a trimmed clip can show more audio than its visible Playlist section. The clip's position, length, fades, gain, stretch settings and routing remain in the Playlist.

For the best result, close the cleanup panel, select at least 100 ms of background noise with no wanted voice or instrument, then open **Tools > Clean up audio (denoise)** and click **Learn selected noise**. Tesla remembers the noise profile for this editor session and clears the selection so the next preview cleans the whole sample. To clean only a portion, close the panel, select that portion and reopen Cleanup. If there is no noise-only section, use **Automatic estimate** and listen carefully; it is an estimate, especially on continuous music.

Start with 12 dB Reduction and 1.50x Sensitivity. Higher Reduction permits stronger attenuation; higher Sensitivity identifies more audio as noise. Click **Preview cleaned** to render and listen. A progress bar appears while processing; Close/Escape cancels processing without editing the sample. Use **Original**, **Preview cleaned** and **Stop** to compare. Preview never changes the document or saved project. Tesla temporarily suppresses its incoming mixer audio during these previews so it does not double up with the audition.

**Apply cleanup** commits the result inside Tesla with one undo step. If you continue editing afterward, the yellow **Apply to track** button at the bottom of Tesla (also in its File menu) returns the edited sample without running denoise again. **Apply to track**, available only when opened from a ML Studio clip, writes a separate cleaned WAV and updates that exact Playlist clip automatically. Other occurrences of the same sample are unchanged. Playlist Undo restores the original clip in one step; the original audio file is never overwritten. The cleaned WAV stays in the project's Rendered/Cleaned folder (or the normal user render folder for projects without a data folder) and is referenced when saving the project. Keep these files with the project when transferring it.

Cleanup uses the current source file. Previously rendered vocal tuning or audio timing is already in that file and stays baked into the cleaned copy; the copied clip does not keep a live link to those earlier editing sessions. Mixer effects are not baked. ARA clips must first be rendered to ordinary audio. If you change the Playlist, source file or source channel while cleanup is open, Apply refuses to replace anything: reopen Cleanup from the intended clip. Changing Tesla's sample length or sample rate also prevents returning it to that clip, because doing so would invalidate its timing. Return links end when the project closes or the operation succeeds.

Tesla also works separately as a **VST3** plugin in FL Studio and other VST3 hosts. Load or drag an audio file into Tesla, use the same cleanup controls, then **Apply cleanup** and **File > Save sample as**. Saved plugin state includes the edited audio. Automatic **Apply to track** is a ML Studio connection; external hosts use Tesla's normal save or drag-out workflow. This is an audio editor that processes the loaded sample, not a live noise gate or an automatic multitrack editor.

This spectral tool targets steady noise such as hiss, fans and hum. It does not separate competing voices or reliably remove individual clicks. Strong settings can damage wanted audio, so compare before applying. Supports mono/stereo, 8–384 kHz, up to 20 minutes and 120 million samples. Noise profiles are not saved across editor sessions. Hide its toolbar tool and Playlist menu action through **Settings > General > Workspace Visibility > Choose what to show > Toolbar extras > Tesla audio cleanup / Playlist cleanup action**; hiding does not change existing audio.
