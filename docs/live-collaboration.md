# Direct-host project collaboration

One participant hosts from the running ML Studio application. No Lemon Studio
website/server, cloud account or separate DAW instance is required. This is live
project editing: each computer renders its own audio with its installed plugins.

## Host and join

1. Both participants use builds with the same collaboration protocol (currently 3).
2. Host: save the project and stop playback. Choose **File > Collaborate**, enter
   your display name and reachable **address:port**, and choose **Host session**.
   Packaging and sample transfer run in a background worker.
3. On the same network, use the displayed LAN address. For distant collaboration,
   use a shared private VPN address, or forward **TCP 32190** on the host router
   and enter the reachable public address. Allow the application through the
   host firewall when prompted. A guest normally needs no inbound port rule.
   Carrier-grade NAT may require a VPN; the app does not provide a relay or
   automatically change router/firewall settings.
4. **Copy invitation** and share it privately with your collaborator. It contains
   a random access key and the host certificate. Anyone holding it can download
   and edit the shared project, so invite trusted collaborators only.
5. Guest: choose **File > Collaborate**, enter your name, paste the invitation,
   and choose **Join session**. Save/discard handling occurs before joining.
   A fresh local project/sample folder is created. Editing the previous project
   while downloading cancels the switch so those edits are retained.
6. Close the panel and work normally. Reopen it for connection status, playback
   following, conflict resolution, recovery files or **End / leave**.

The host must remain open and connected. Ending hosting invalidates the invitation;
participants retain their local projects. Nothing listens on the network at normal
application startup. Workspace Visibility can hide the inactive Collaborate menu;
active sessions keep it accessible so they can always be ended.

## Shared editing

- Patterns, steps, notes, note properties, channel event controls and metadata.
- Playlist clips, placement, trim/properties, lane metadata, arrangements and markers.
- New/deleted channels, instruments, audio sources and automation channels/points.
- Mixer controls, routing and sends, effect slots, channel settings and project tempo.
- Hosted plugin parameter gestures. Full plugin state/preset snapshots are captured
  after edits settle while playback is stopped. Plugin replacements/state loading
  received during playback are deferred until that computer stops.
- Additional project audio files introduced after joining. Transfers use hashes,
  bounded chunks, resumable partial files and per-computer cached paths.

Edits are sent after completed local gestures. Separate pattern channels, Playlist
lanes and individual Mixer controls can merge independently. Changes to the same
shared item are checked against the previous value, rather than silently overwriting
another person's edit. Related structural changes are applied as one operation.
Remote Mixer edits preserve unrelated note/clip selections; selections tied to
changed/deleted note or clip arrays are cleared safely.

Plugin binaries, licences and external plugin libraries are not transferred.
Both computers need compatible plugins/libraries. Plugin parameters must be exposed
to the host to synchronize live. Stop playback on both computers for full preset or
plugin-instance changes. Identical project data does not guarantee identical sound
when plugins, external libraries or device settings differ.

## Playback and local workspace

Playback is independent by default. A guest can enable **Follow host playback**
for host start/stop, song/pattern position and periodic drift correction. This is
optional position following, not sample-accurate synchronization, an internet jam,
voice chat or audio streaming. Network delay and each computer's audio buffer affect
what the participants hear. Disabling Follow restores independent transport.

Device choices, window layout, zoom/scroll, active editor choices, Playlist lane
heights and folding remain local. Networking, transfer, hashing and recovery writes
run outside audio processing. UI application of remote changes waits until local
gestures or unrelated modal dialogs have finished.

## Conflicts, disconnects and recovery

A conflicting edit keeps the session connected and pauses that participant's shared
editing. Choose **Keep my version** or **Use shared version** in Collaborate. A native
**Before conflict ... .lmn** copy is saved before resolution; failure to save prevents
the resolution from discarding that local version. Keep my version reapplies the
local differences over the latest shared project; impossible structural merges
require saving a copy and using the shared version.

Local Undo refuses to erase a snapshot that a collaborator has since changed.
Session start/end clear pre-session Undo history. This is deliberately conservative;
not every grouped old edit can be undone after remote changes.

Temporary disconnections trigger reconnect attempts with operation identities that
prevent duplicate application. A prolonged failure ends synchronization, retains
local work and attempts to save **Recovered local project.lmn**. Local playback is
not stopped merely by a network error. After the host ends, start a new session
rather than reusing its old invitation.

Use **Open recovery folder** to find the initial package, pending/conflicting edit
journals and **Latest shared project.lmn** / **Previous shared project.lmn** checkpoints.
Guest joining also preserves a before-joining checkpoint. Recovery writes are best
 effort: disk/permission errors can still prevent writing, so save normally too.
Recovery folders contain project data and are intentionally excluded from automatic
build cleanup. Keep any wanted recovery project and its samples together.

## Limits and connection troubleshooting

- Initial package: at most 512 MiB compressed / 1 GiB expanded.
- Individual transferred audio file: at most 512 MiB; session asset storage: 4 GiB.
- Shared document: at most 24 MiB. Messages, queues and project collections are bounded.
- TLS pins the host certificate from the invitation. Unsupported protocol versions,
  invalid paths, bad file hashes and invalid shared edits are rejected.

If joining fails, check the displayed host address/port, host running state and
firewall permissions. Across the internet, test your VPN/port forwarding and ISP
reachability. Being able to connect on the same Wi-Fi does not verify public-internet
reachability. **Connection help** is also available inside Collaborate.

## Candidate verification

This implementation is being prepared for the next update; it is not published by
building or installing Development. Focused tests cover two application instances,
project/sample transfer, notes/Playlist/Mixer/tempo/automation changes, concurrent edits,
conflicts and resolution, guarded Undo, selection preservation, encrypted peers,
reconnect deduplication, hash validation and unsafe package-path rejection.

Actual Windows/Mac peers have been checked in both hosting directions on the LAN.
The release handoff records the final platform build/test results. Public-internet
NAT/firewall conditions and hands-on third-party-plugin/audio-device behavior still
need testing on the users' actual networks and systems; LAN checks do not establish
those results.
