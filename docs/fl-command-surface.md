# FL Studio 26.1.7 — hela kommandoytan

Alla **6,256** menyliknande strängar ur `FL64.exe` (block 31 i payloaden, 22,2 MB), filtrerade ur 225 791 ASCII- och 16 231 UTF-16-strängar och sorterade i fem fack.
Inget är bortvalt: varje rad nedan finns ordagrant i binären.

| Fack | Rader |
| :--- | ---: |
| Kommandon (gör något) | 1642 |
| Etiketter och parameternamn | 3252 |
| Meningar (ledtrådar/status) | 992 |
| Interna handlernamn (avslöjar funktionsområden) | 5 |
| Fel- och kontrollsträngar | 365 |
| **Summa** | **6256** |

Filtret som gav de 6 256 raderna: 3–48 tecken, inleds med versal, inga sökvägar, inga
`%s`/`%d`-format, inga `_`-identifierare. Det betyder att enstaka ord (t.ex. `Chorus`) faller
bort och att interna handlernamn finns kvar i sitt eget fack.

---

## Kommandon (gör något) — 1642

```
Adapting spectrum...
Add audio time marker
Add Audio track
Add audio track
Add auto
Add channel
Add content
Add control
Add folder to tab
Add from state
Add jump to next bar
Add key marker
Add marker...
Add meshes
Add meter
Add missing search folders
ADD MODE
Add multiple...
Add NDI
Add new chord
Add notes to PR
Add one
Add one every
Add pictures
Add plugin
Add plugins
Add plugins to list
Add points at audio markers
Add points at regions
Add points at selection
Add preset
Add region...
Add Sample(s)
Add sample(s)...
Add Spout (beta)
Add step freeze
Add steps
Add swing
Add tag
Add tail
Add text line
Add the plugin to the database, as a favorite
Add this folder to favorites
Add time signature change...
Add time signature...
Add to cart
Add to key
Add to plugin database (flag as favorite)
Add two...
Add URL
Add videos
Add WebCam
Add Window
Adjust position of recorded audio
Adjust scaling factor...
Advanced fill...
Align all channel editors
Align block size
Align tick lengths
All...
Amp...
Analysing sample, please wait...
Analyze audio file...
Analyze input file
Analyze single-cycle waveform...
Analyze timbre in audio clipboard
Analyze timbre...
Analyzing...
Append to existing score
Apply changes
Apply selected pattern to all
Apply tempo
Apply track level and mute when recording
Arm recording feature not available
Arm selected track(s)
Arpeggiate...
Articulate...
Assign all to
Assign both decks to black  white notes
Assign both decks to whole keyboard
Assign current region to this key
Assign deck A regions to articulator 1
Assign deck B regions to articulator 2
Assign Note
Assign note
Assign one layer per key (starts at root note)
Assign selected to free mixer track(s)
Assign to a free track
Assign to free mixer track
Assign to free tracks
Assign to mixer
Assign to new instrument track
Assign to new mixer tracks
Assign trigger notes to all...
Autodetect...
Blur...
Browse all installed plugins
Browse channel states
Browse for Folder
Browse for folder
Browse for project data folder
Browse for registration file...
Browse for sample...
Browse for the project data folder
Browse for tool...
Browse in FL Cloud...
Browse library of free online images and videos
Browse note color presets
Browse note name presets
Browse parameters
Browse plugin database
Browse presets
Browse presets...
Browse states
Browse...
Building spectrum...
Buy now...
Calculating ...
Calculating resampling lookup tables...
Change browser directory
Change color
Change color...
Change default template...
Change icon
Change icon...
Change LS pattern name
Change program name
Change to DeviceID while device was open
Change to loop channel
Change to marker loop
Change to ProductName while device was open
Change to steps channel
Channel isn't active...
Checking for language updates...
Checking for MIDI script updates...
Choose a description for your track
Choose a Genre you like from the list.
Choose a name
Choose a previous pattern during this session
Choose a title for your track
Choose another project to open
Choose to preview final output or a buffer
Choose which tags to use
Chop chords
Chop in 16th beats
Chop in 32nd beats
Chop in 8th beats
Chop in bars
Chop in beats
Chop in half beats
Chop in quarter beats
Chop into regions
Chop using sample built-in slicing information
Chop...
Chord progression tool...
Claw machine...
Clean up (denoise)...
Clear all
Clear log
Clear output
Clear pad
Clear panel
Clear pattern
Clear playlist and create new project
Clear selection
Click here to update the search results...
Clone as region
Clone for further editing
Clone layer
Clone pattern
Clone playlist tracks warning
Clone selected playlist track(s)
Clone this tab
Clone with links
Clone...
Close active popup menu
Close all menu
Close all panels
Close all plugin windows
Close all unfocused windows
Close all windows
Close anyway
Close graph editor
Computing envelope...
Computing impulse...
Computing segments...
Connecting to news server...
Consolidate this track
Consolidate track(s)
Continue to render and save video...
Convert left channel to mono
Convert mono signal to mono format
Convert right channel to mono
Convert shape to sine harmonics
Convert to automation
Convert to image synthesis mode
Convert to score and dump to piano roll
Convolution reverb...
Copy current part to the Windows clipboard
Copy error to clipboard
Copy from pitch
Copy layer
Copy link
Copy oscillator settings
Copy part
Copy Patch
Copy preset
Copy preset to the Windows clipboard
Copy Sequence
Copy the data
Copy to all zones
Copy to audio clipboard
Copy to bitmap clipboard
Copy to clipboard
Copy to image clipboard
Copy to MIDI clipboard
Copy to part
Copy to part B
Copy to selected zones
Copy to the data folder
Copy value
Create a new empty project data folder
Create a new project folder
Create a new tab with search results
Create account
Create an account
Create auto fades
Create automation clip
Create automation clips
Create bouncing loop time envelope
Create crossfade
Create DirectWave instrument...
Create DW presets
Create events
Create free FL Cloud account
Create group...
Create MIDI clock receiver
Create new clips with manual fades
Create new project folder
Create one channel per track
Create overlay map
Create pad harmonics
Create pad harmonics...
Create program from samples...
Create refresh thread
Create sequence
Create sequence...
Create submix...
Create support ticket
Custom size...
Custom...
CUT + CUT ENV
Cut By
Cut group
Cut itself
Cut mode
Cut playing notes
Cut remainder
Cut self
Decimate points...
Decimating envelope points...
Declipping, please wait...
Declipping...
Delete a line to delete the layer.
Delete all
Delete all children channels
Delete all links
Delete all markers
Delete all zones
Delete associated automation and audio clips
Delete automation target link
Delete automations
Delete channel
Delete channels
Delete children...
Delete effect plugin
Delete file in FL Browser  warning
Delete file...
Delete instrument channel
Delete layer
Delete mixer track
Delete part after loop
Delete pattern
Delete playlist arrangement
Delete playlist track
Delete preset
Delete recorded files to recycle bin
Delete selected...
Delete space
Delete sub tracks
Delete this tab
Delete track
Delete tracks and channels
Delete unused recorded files
Delete zone(s)
Delete...
Deleting channels...
Deleting unused sample files...
Denoising...
Detect beats
Detect pitch regions
Detecting beats...
Detecting downbeat...
Detecting middle note...
Detecting pitch regions...
Detecting score...
Detecting tempo...
Detecting...
Deverb...
Disable FL CLOUD search
Disable max poly
Disable maximum polyphony
Disable sync
Distribution...
Download finished, extracting...
Downloading ...
Downloading FL Studio update ...
Downloading optional features...
Downloading...
Drag and drop feature not available
Drag controls to and from the toolbar
Drag to move (can't be removed)
Drag to move...
Drag to rearrange
Drop audio on track headers
Drop on PL track
Drop presets on multiple mixer tracks
Drop replace drum
DROP SAMPLE HERE
Drum(loop) stretch...
Dump normally
Dump score log
Dump score log to selected pattern
Dump to audio editor
Dump to piano roll
Dumping score...
Duplicate pattern data
Duplicate transaction id
Duplicate zone
Edit articulator
Edit automation clip...
Edit effect in ZGameEditor
Edit envelope...
Edit events in new window
Edit events in piano roll
Edit image
Edit in audio editor
Edit in new window
Edit in pitch corrector
Edit in time warper
Edit last script
Edit link must not be nil.
Edit name
Edit palette...
Edit panning envelope
Edit patterns in
Edit patterns in channel rack
Edit patterns in piano roll
Edit properties...
Edit sample...
Edit script
Edit script...
Edit slot name...
Edit step colours
Edit stereo separation envelope
Edit Strings
Edit strings...
Edit stutter pattern...
Edit tags
Edit volume envelope
Edit waveform
Edit whole mapping
Enable debug log
Enable device
Enable DMX output
Enable FL Studio Remote
Enable gamepads
Enable Image-Line Remote
Enable indexing
Enable insert effects
Enable internal controllers
Enable loop starter
Enable main pitch
Enable master effects
Enable mastering
Enable mastering reference A
Enable mastering reference B
Enable MIDI input
Enable MIDI master sync
Enable MIDI output
Enable MIDI remote control
Enable MIDI sync
Enable NDI output
Enable online news
Enable portamento
Enable Spout output (beta)
Enable undo for large samples
Enable video preloading
Enter preset number...
Enter random preset number...
Equalize...
Errr ...
Explore script folder...
Export all playlist track groups
Export all playlist tracks
Export as score sheet
Export display as...
Export Flac
Export kit as SFZ file...
Export M4a
Export MP3
Export OGG
Export project bones
Export regions
Export score as MIDI file...
Export single-cycle waveform...
Export to text file...
Export type
Export Video
Export WAV
Fill color
Fill color (pressed)
Fill each 2 steps
Fill each 4 steps
Fill each 8 steps
Fill every
Fill every N steps
Fill gaps
Fill rail
Fill rectangle
Fill ruler
Fill type
FILTER 1
Filter 1
Filter 1  2 mix
Filter 1 envelope amount
Filter 1 frequency
Filter 1 mask
Filter 1 resonance amount
Filter 1 resonance mask
Filter 1 resonance offset
Filter 1 resonance width
Filter 1 width
FILTER 2
Filter 2
Filter 2 envelope amount
Filter 2 frequency
Filter 2 mask
Filter 2 resonance amount
Filter 2 resonance mask
Filter 2 resonance offset
Filter 2 resonance width
Filter 2 width
FILTER 3
Filter 3
FILTER CUT
Filter Cutoff
Filter cutoff
Filter Env
FILTER ENVELOPE
Filter envelope
Filter frequency
Filter frequency knob
Filter group name
Filter Resonance
Filter resonance
Filter shape 1
Filter shape 2
Filter tags
Filter unharmonics - narrow
Filter unharmonics - normal
Filter unharmonics - wide
Filter unit
Filter...
Filtering...
Finalizing...
Find a script editor...
Find a tool...
Find event id
Find files
Find first empty
Find first empty...
Find first next empty pattern
Find installed plugins
Find it on Google
Find media online
Find next
Find next empty (no naming)
Find next empty...
Find other files
Find pitch root...
Find plugins
Find previous
Find samples that match the loop in FL Cloud
Find similar samples...
Find templates
Find text line
Finding  deleting channels...
Finishing...
Flam...
Flatten (groove)
Flip...
For common use...
For sampler use...
Freeze all
From track start...
Generate chord progression...
Generating image...
Get active pattern group
Get all future updates free, register your box
Get block set status
Get block status simple
Get browser auto hide state
Get changed flag
Get channel activity level
Get channel color
Get channel grid bit
Get channel index
Get channel loop style
Get channel midi in port
Get channel mute state
Get channel name
Get channel number
Get channel pan
Get channel pitch
Get channel rec event id
Get channel selected state
Get channel solo state
Get channel target FX track
Get channel volume
Get channels count
Get closing state
Get current pattern number
Get current step param
Get current tempo
Get device hardware id
Get device master sync
Get device name
Get dispatch midi message receiver count
Get display zone number
Get eq band count
Get eq bandwidth
Get eq frequency
Get eq gain
Get event ID name
Get event ID value string
Get event value from midi
Get FL Studio hint
Get FL Studio title
Get FL Studio version as string
Get focused form caption
Get focused form ID
Get focused node caption
Get focused node file type
Get focused plugin name
Get form focus state
Get form visible state
Get grid bit with loop
Get hint value
Get idle elapsed
Get last peak vol
Get launchmap item aftertouch
Get launchmap item color
Get launchmap item destination channel
Get launchmap items length
Get launchmap pages length
Get linked control info back or -1 if not linked
Get live block color
Get live block status
Get live block status simple
Get live block status simplest
Get live loop mode
Get live pos snap
Get live status
Get live trig snap
Get live trigger mode
Get loop mode
Get marker name
Get max pattern id
Get Midi scripting version
Get normalized song position
Get Patches
Get pattern color
Get pattern default state
Get pattern group count
Get pattern group name
Get pattern length
Get pattern name
Get pattern selected state
Get patterns count
Get patterns in group
Get performance mode state
Get plugin color
Get plugin name
Get plugin pad info
Get plugin parameter count
Get plugin parameter name
Get plugin parameter value
Get plugin parameter value string
Get plugin validity
Get precounr
Get preset count
Get project author
Get project genre
Get project title
Get rec ppb
Get Rec ppq
Get rec ppq
Get Rec pps
Get receiver port number
Get recording state
Get route send state
Get route to lebel
Get screen active counter
Get selected channel number
Get sing tick pos
Get slot color
Get snap mode
Get song length
Get song position
Get song position hint
Get song start tick position
Get song step position
Get started with the essentials.
Get step edit mode
Get step param
Get swing
Get time bar
Get time step
Get time tick
Get track activity level
Get track armed state
Get track autotimation state
Get track color
Get track count
Get track dock
Get track enabled state
Get track mute lock state
Get track mute state
Get track name
Get track number
Get track pan
Get track peaks
Get track plugin ID
Get track plugin mix level
Get track plugin mute state
Get track plugin valid state
Get track recording file name
Get track reversed polarity state
Get track selected state
Get track slots enabled state
Get track solo state
Get track stereo separation
Get track swap channels state
Get track visual activity level
Get track volume
Get tracks count
Get tracks info
Get undo history count
Get undo history last
Get undo history position
Get undo level
Get use metronome
Ghost producer...
Gradient color...
GROUP 1
GROUP 2
GROUP 3
GROUP 4
Group children...
Group loop recorded takes
Group name
Group notes
Group playlist tracks
Group tracks
Hide all plugin windows
Hide all plugin windows on close
Hide collapsed grouped tracks
Hide menu
Hide plugin window toolbar by default
Hide this message now and remind me later
Hide this tab. It can be shown again later
Hide window
Import cue points from wav
Import DirectWave program...
Import from text file...
Import MIDI data
Import SYSEX...
Import tags
Import time signatures
Import to content library
Import wallpaper
Import Yamaha DX7 preset bank
Import zero velocity notes
Increase number of tracks to...
Initializing...
Insert current controller value
Insert from state
Insert layer
Insert Loop Starter loop channel
Insert Loop Starter steps channel
Insert multiple...
Insert new slice here
Insert one
Insert space
Insert tails
Install for all users
Install license file to unlock FL Studio.
Install now
Last exported format(s)...
Learn how to fix this...
Learn more...
LFO...
Limit...
Link all parameters...
Link channel to track
Link controller control to last tweked parameter
Link key to
Link key to pitch
Link key to timbre seed 1
Link note on velocity to
Link release to release length
Link release velocity to
Link timbre release to volume decay
Link track to channel
Link velocity
Link velocity to attack length
Link velocity to attack time
Link velocity to gain
Link velocity to volume
Link with Surface
Load a demo project
Load a new preset
Load bitmaps
Load groove pattern...
Load layer from file
Load license file...
Load MRU list
Load next library program
Load note layout...
Load oscillator shape...
Load Patch
Load plugin tags
Load preset...
Load previous library program
Load regions
Load sample...
Load slice markers
Load...
Loading ...
Loading data...
Loading dialog...
Loading module...
Loading plugin list...
Loading plugin state...
Loading sample preview...
Loading sample...
Loading tags started...
Loading your FL Studio Web projects ...
Loading your Project backups ...
LOADING...
Locate missing file...
Log out...
Loop all channels
Loop End
Loop end
Loop half of regions
Loop markers
Loop off
Loop record
Loop recording
Loop Start
Loop start
Loop Starter
Loop starter
Loop starter channel dice
Loop Starter closed
Loop starter dice
Loop Starter opened
Loop Starter scripts
Loop starter scripts
Loop starter send to playlist
Loop starter tutorial
Loop starter update
Loop step channels
Loop tuner
Loop Type
Loop type
Manage accounts...
Manage cloud accounts...
Manage cloud plugins...
Manage FL Cloud plugins...
Map audio markers to keys
Map audio regions to keys
Map note color to MIDI channel
Map notes for entire bank
Map time offset to last hit key
Master...
Mastering...
Match spectrum...
Merge all links
Merge arrangement
Merge automation clips
Merge pattern clips
Merge selected clips
Merge similar pattern clips
MIDI devices have changed... refreshing...
Mixing...
More colors...
More information...
More plugins...
More unlock options...
More...
Move content left
Move content right
Move down
Move down in undo history
Move layer left
Move layer right
Move left
Move markers to their nearest zero-crossing
Move pattern
Move right
Move selected down
Move selected up
Move slice points to their nearest zero-crossing
Move slot left
Move slot right
Move tab
Move tab to the left
Move tab to the right
Move up
Move up and down in undo history
Move up in undo history
MP3 file...
Multilink...
Mute all
Mute all clips
Mute group
MUTE IN
Mute selected
Mute source clip
Mute source track
My account...
New Age
New audio editor
New firmware is available for your MIDI device
New from template
New key kills all keys
New key only kills same key
New key releases all keys
New link
New MIDI device detected
New mixer track(s)
New pattern
New pattern (find first
New project
New tracklist
New version
New version available
New version of script has been downloaded
New version of script is available
New Wave
New zone
Normalize all regions
Normalize as a group
Normalize layered region levels
Normalize levels
Normalize loudness
Normalize selection as a group
Ok, might buy later...
Open (new) audio editor
Open account settings
Open all demo projects
Open all native plugins
Open all panels
Open all presets for all native plugins
Open audio editor
Open audio editor and record
Open audio file
Open audio logger
Open background image
Open Edison for further editing
Open event edit
Open event editor
Open FL Studio Web
Open FL Studio window
Open image file
Open image file...
Open in BeatSlicer
Open in browser
Open in DirectWave
Open in Edison
Open in FL Cloud
Open in new channel
Open in new FLEX channel
Open in new Fruity Granulizer channel
Open in new Fruity Slicer 2 channel
Open in new Fruity Slicer channel
Open in new sampler channel
Open in new Slicex channel
Open in project browser
Open MIDI settings.
Open NewStuff project
Open optional feature
Open other...
Open parent folder
Open plugin picker
Open program or bank file
Open project
Open Sample File(s)
Open script folder...
Open shop
Open single-cycle audio file
Open sound file(s)
Open the FL Cloud Plugins application.
Open the project
Open the support forums in a web browser
Open the Unlock window.
Open this project
Open unlock window
Open your FL Studio Web projects here
Opening denoiser...
Opening file...
Other projects...
Override volatile link...
PAN + PAN ENV
Pan fading
Pan flute
Parsing MIDI data...
Paste (replace)
Paste from bitmap clipboard
Paste from image clipboard
Paste layer
Paste oscillator settings
Paste Patch
Paste preset
Paste preset from the Windows clipboard
Paste replace drum
Paste Sequence
Paste stretch
Paste Succeeded
Paste value
Pause step
Play Mode
Play sample
Play startup sound
PLAY TO END
Play truncated notes in clips
Play truncated notes on transport
Please tweak the controls you want to link...
Precounting...
Preloading video...
Preparing download...
Preparing resynthesis...
Processing amp envelopes...
Processing mix envelope...
Processing scratching envelope...
Processing source...
Processing target...
Processing...
Project bones...
Project data files...
Project loaded...
Properties...
Quantize duration
Quantize end time
Quantize ending time
Quantize time
Quantize times
Quantize using main snap value
Quantize...
Quick rename all...
Randomize all parameters
Randomize bands
Randomize levels
Randomize matrix
Randomize next preset with previous one
Randomize phases
Randomize slices
Randomize waveform settings
Randomize...
Record automation
Record detected input
Record to step sequencer
Record when host is playing
Redeem code...
Refresh device list
Refresh list
Refresh live clips
Refresh plugin list (fast scan)
Refresh plugin properties
Refreshing cache ...
Refreshing FL Cloud cache ...
Remix a song...
Remove 1 note
Remove all Edison instances
Remove background picture
Remove clips
Remove conflicts
Remove DC offset
Remove every 2
Remove every 3
Remove every 4
Remove every 5
Remove folder from tab
Remove folders that don't exist
Remove Gibbs phenomenon
Remove link
Remove meshes
Remove notification
Remove ringing ripples
Remove selected
Remove selected slice
Remove short notes
Remove text line
Remove this folder from favorites
Remove track route warning
Remove unused
Remove your FL Studio license from this computer
Rename all...
Rename C0
Rename data folder
Rename group...
Rename layer
Rename project
Rename the project
Rename, color and icon...
Rename...
Render (master)
Render a 2D net
Render and replace...
Render as audio clip...
Render as audio file...
Render at 2x resolution and downsample
Render audio to file
Render loop to WAV file
Render minimized
Render options
Render plugins
Render selected tracks to wave file(s)...
Rendering instrument...
Replace all
Replace all samples
Replace clashing
Replace file
Replace sample
Replace sample(s)...
Replace Samples(s)
Replace the selected file with another file
Resampling...
Reset all
Reset all bands
Reset before processing
Reset browser cache
Reset children
Reset crossfade tension
Reset custom stretch
Reset defaults
Reset editor size
Reset fade times to 0
Reset fades
Reset FL Cloud cache
Reset folder location
Reset history
Reset latency values in mixer
Reset offset
Reset on transport
Reset parameter value
Reset PDC
Reset phases
Reset plugin when FL Studio resets
Reset plugins on transport
Reset project data folder
Reset selected mixer track(s)
Reset selected track(s) routing
Reset selected track(s) to default
Reset stretch factor
Reset the associated mixer track
Reset to 0
Reset to defaults
Reset user data location
Reset waveform settings
Reset working time
Restore default shortcut
Restore more window settings
Restore previous state after solo
Restore settings for most windows
Restore settings window
Restore undo
Restore undo by level
Restore window state
Retrieving registration code, please wait...
Retrieving...
Reverse all
Reverse clip
Reverse polarity
Reverse selected audio clips
Reverse selected clips
Reverse slices
Revert to
Revert to last backup
Riff machine...
Route selected channels starting from this track
Route selected channels to this track
Route selected to this track
Route selected to this track only
Route this channel to an empty mixer track
Route this track to selected
Route this track to selected only
Route to this track
Route to this track only
Running MulDiv test...
Save a preset in Cubase format (fxp)
Save all wave files...
Save all...
Save analysis files
Save and back up to FL Cloud
Save and back up to FL Cloud warning
Save and restart
Save anyway
Save as
Save as image file...
Save as MIDI...
Save as template...
Save as Variation
Save as variation...
Save as...
Save audio clipboard to browser
Save audio file
Save clipboard sample
Save entire kit as pad presets...
Save for FL Studio Mobile...
Save groove pattern...
Save image file
Save in history
Save in presets
Save kit as presets
Save layer to file
Save loop markers
Save MIDI clipboard to browser
Save modified sample
Save new version
Save note layout...
Save note markers
Save now
Save only the project
Save original sample...
Save playlist markers
Save plugin version
Save preset
Save preset as...
Save preset...
Save presets...
Save processed sample...
Save project
Save sample
Save sample as 24Bit .wav file...
Save sample as 24bit .wav file...
Save sample as...
Save sample selection
Save search result as tab
Save single-cycle audio file
Save still image
Save tempo
Save tempo information
Save the project
Save to new project folder...
Save to project folder
Save to the folder
Save undo
Save validation file...
Save wav type
Save wave files
Save...
Saving channels...
Saving header...
Saving image...
Saving tracks...
Saving...
Scale editor dimensions
Scale levels...
Scale options are unavailable when
Scale Snapping is enabled in Piano Roll
SCALE STEP
Scan - internal
Scan options
Scanning channels...
Scanning patterns...
Scanning plugin database (effects)...
Scanning plugin database (generators)...
Scanning...
Search box on top
Search canceled
Search FL Cloud
Search for
Search for folders
Search media online
Search paths
Search pattern is empty
See the news on our website...
Select a directory
Select a directory to browse for samples...
Select a folder
Select a folder to consolidate to
Select a image or video
Select a layer to load
Select a theme preset
Select a wav-file
Select after current selection
Select all
Select all (slices)
Select all (time and slices)
Select all (time)
Select all channels
Select all patterns
Select all similar clips
Select all tracks
Select an edition to trial
Select at least one stem to continue.
Select before current selection
Select browser menu item
Select by color
Select by selected source
Select by selected source has no selection
Select camera
Select channel exclusively
Select chord
Select color and paste it into HTML text
Select compressed format
Select Directory
Select effect plugin
Select file
Select frame rate
Select generator plugin
Select in playlist
Select linked channels
Select loop starter genre
Select mod LFO 1 waveform
Select mod LFO 2 waveform
Select more at random
Select muted
Select next menu
Select next plugin preset
Select next region
Select next time
Select odd
Select one at random
Select one or more meshes
Select one or more videos
Select or create a new empty project data folder
Select or create a project folder
Select overlapping clips
Select overlapping notes
Select patch
Select pattern
Select percussive patch
Select play mode
Select playlist tool
Select plugin
Select presets to import
Select previous menu
Select previous plugin preset
Select previous region
Select previous time
Select reference file
Select region
Select region assigned to this key
Select rendering folder
Select SoundFont editing tool...
Select stacked clips
Select stacked notes
Select theme...
Select time around selection
Select unused
Select what is included when recording audio
Select where to save the image
Select where to save the layer
Select where to save the video
Select window
Select zoomed part
Send  generic midi message to active plugin
Send all control values now
Send all notes off
Send all values now
Send audio to playlist
Send children to a group
SEND FX
Send loop position
Send master sync
Send midi out sysex
Send MIDI pitch
Send MIDI pitch bend range
Send MIDI reset
Send MOD X as polyphonic aftertouch
Send new code
Send next to selected channel
Send note release velocity
SEND ONLY
Send out generic midi message
Send out midi message
Send out new midi message
Send pan
Send pan, volume and pitch
Send param changes
Send pattern instances here
Send pitch
Send pitch bend range
Send previous to selected channel
SEND TO
Send to
Send to devices
Send to piano roll
Send to playlist as an audio clip
Send to playlist as an audio track
Send to playlist as an instrument track
Send to playlist as audio clip
Send to selected channel
Send usage data
Send value
Send volume
Set active track
Set all cut groups to...
Set all levels to value
Set all to fixed
Set all to off
Set as time division
Set as time signature
Set background color...
Set bitmap time
Set browser auto hide state
Set channel color
Set channel grid bit
Set channel loop
Set channel name
Set channel pan
Set channel pitch
Set channel selected state
Set channel target FX track
Set channel volume
Set children
Set chord inversion
Set current tempo
Set defaults
Set denominator
Set device master sync
Set dynamic wallpaper
Set eq bandwidth
Set eq frequency
Set eq gain
Set existing channel as instrument track
Set first downbeat
Set FL Studio hint
Set form focus state
Set from
Set has meters
Set height
Set image wallpaper...
Set in beats
Set in ms
Set in samples
Set info...
Set last played zone as loop region
Set loop
Set loop mode
Set LS Loop mixer track name and color
Set LS Steps mixer track name and color
Set map item destination target
Set midi note on for channel
Set mixer tracks for new channels
Set normalized song position
Set numerator
Set pattern color
Set pattern length
Set pattern name
Set pitch
Set pitch...
Set playback speed
Set plugin parameter value
Set rec ppq
Set route to
Set route to lebel
Set screen active counter
Set selection start
Set Size Exceeded.
Set slot color
Set snap mode
Set song position
Set step edit mode
Set step parameter by index
Set swing
Set swing mix for selected
Set tempo-sync
Set the current OpenGL color
Set the master audio output target
Set time signature...
Set to current
Set to current project's folder
Set to default patch
Set to next bar
Set to next beat
Set track color
Set track name
Set track number
Set track pan
Set track plugin mix level
Set track plugin mute state
Set track selected state
Set track setereo separation
Set undo history count
Set undo history last
Set undo history position
Set video end time
Set video start time
Set viewed zone as loop region
Set width
Set...
Setting things up...
Settings...
Show all
Show all hidden tabs
Show all installed plugins
Show all unlock options again
Show ASIO panel
Show avail mem
Show available
Show average instead of peaks
Show background pattern in preview
Show buttons
Show channel rack or playlist
Show channel settings
Show children
Show chord alternatives
Show chord name for selection
Show clip properties
Show complete piano roll preview
Show controls on audio tracks
Show cs form
Show empty patterns
Show every time
Show fade previews
Show file
Show file extensions
Show file(s) when complete
Show files
Show FL Studio notification
Show FL Studio VSTi window
Show folder icons
Show forever
Show gain previews
Show gain scale
Show gain value
Show ghost notes
Show GM note names
Show graph editor
Show help for this effect
Show help in a panel
Show help in a web browser
Show hidden
Show icons and text on tabs
Show images
Show in browser
Show info on opening
Show labels
Show levels on audio tracks
Show levels on instrument tracks
Show library tree
Show license plate
Show load progress
Show me
Show me some more help
Show menu when enabling Loop Starter
Show MIDI import window when necessary
Show missing files
Show more from this pack...
Show none
Show note names in host
Show on right side
Show only one folder content
Show only one panel
Show only opened folders
Show pack in Sounds tab
Show pack in sounds tab
Show panel
Show panel locks
Show parent folders
Show parent folders in search results
SHOW PEAKMETERS
SHOW PEAKS
Show picker panel
Show plugin database
Show plugin picker
Show preview
Show Python output
Show recent changes at the top
Show sample waveform
Show startup splash screen
Show the list of downloads
Show the news panel
Show the notifications panel
Show this pack in FL Cloud
Show transparency
Show unknown file types
Show used
Show VSTi panel
Show waveform preview
Show when enabling Loop Starter
Show when necessary
Show window
Slice  insert space
Slice in beats
Slice map
Slice map mode
Slice preview
Slice stretch
Slicing...
Slow download...
Smart find in this folder...
Smooth up...
Snap 1
Snap 2
Snap 3
Snap 4
Snap 5
Snap 6
Snap 7
Snap 8
Snap 9
Snap all controls to grid
Snap all devices
Snap downbeat
Snap existing notes
Snap fade handles
Snap handle
Snap incoming MIDI
Snap PR notes to scale
Snap ruler
Snap to grid
Snap to grid helping cells
Snap to grid helping lines
Snap to markers
Snap to other events
Snap to pitch period
Snap to regions
Snap to samples
Snap to scale
Snap to zero-crossing
Solo layer
Solo selected track(s)
Sort by
Sort by color
Sort by mixer track number
Sort by name
Sort files by
SPLIT BALANCE
Split Balance
Split children
SPLIT FREQ.
Split Frequency
Split mixer tracks
Stop on
Stop playback
Stop recording
Stop when there's no more sound
Stretching slices...
Stretching...
Strum...
Switch all audio clips to
Switch all plugin editors to
Switch program channel
Switch smart disable for all plugins
Tapping...
The update is now downloading...
Time selection...
Transpose down
Transpose one octave down
Transpose one octave up
Transpose selected...
Transpose semitone
Transpose sequences
Transpose up
Transpose...
Tune loop...
Tune pitch for time offset
Tune Request
Turn into automation clip...
Type in chord name...
Undo backwards
Undo History
Undo history
Undo knob tweaks
Undo level
Undo make automation channel unique
Undo mix (envelope)
Undo mix (spectral)
Undo tests are disabled in Trial mode
Ungroup selected...
Unlink channel
Unlink channel and mixer track
Unlink channels
Unlink track
Unlink tracks and channels
Unlock with file...
Unzip used files...
Update browser
Update channel rack
Update favorite plugins
Update for Loop Starter has been downloaded
Update graph editor
Update languages
Update launchmap page
Update license
Update MIDI scripts
Update notification
Update on close
Update registration
Update screen
Update toolbars and stuff
Update tools menu
Update when hidden
Update your expired license
Update your license
Update your license before it expires
Updating sample pool...
Upgrade to Producer edition for more features...
Upgrade to unlock all languages...
Upload samples...
Upload to cloud...
Upload to YouTube...
Upload...
Video file...
Warping...
Zip used files...
Zoom in
Zoom on
Zoom on left of selection
Zoom on performance clips
Zoom on right of selection
Zoom on selection
Zoom out
Zoom out far
Zoom out full
```

---

## Etiketter och parameternamn — 3252

```
A -
About Harmor
Above Nyquist
Abs Sine
ABSOLUTE C4
ABSOLUTE C5
ABSOLUTE C6
Absolute controller
Absolute knob
Absolute mode
Absolute pattern
Accent mode OFF
Accent mode ON
Accentuate beat
Accentuate beat-aligned slices
Accept dropped files
Accidental Mode
Account settings
Acid Jazz
Acid Punk
Acquire noise profile
Acquire noise threshold
Active 1O
Active color
Active Sensing
Active step indicator
AddAudioTrackMenu +
Added at
Advanced edit pitch
Advanced edit vibrato
Advanced export settings
Advanced fill
Advanced settings
After routing changed
Aftertouch (Wrapper)
Agreed to terms
AI input parameters
Air Bombing
Akai APC
Akai APC20
Akai APC40
Alesis Photon X25
Alien Blue
All (gain-compensated)
All aligned
All audio clips
All automations
All channels unmuted
All patterns
All Plugins
All Plugins Edition
All tracks
Allow clips nudging
Allow smart disable
Allow threaded processing
ALT DECAY
Alt flanger
Alt key modifier
ALT x2
ALT x3
Alternate direction
Alternate fill gaps
Alternate flipping
Alternate meter scale
Alternate undo mode
Alternative mixer highlighting
Always ask
Always on top
AM OSC 3
Amazon Music
AMP DECAY
Amp Env Att
Amp Env Dec
Amp Env Rel
Amp Env Sus
Amp Follower
Analysis context
Analysis failed
Analyzer - Left
Analyzer - Mono
Analyzer - Right
Animate text
Animation refresh rate
Antique White
Apple + Intel
Apple Loops
Apple Music
Arial Black
Around fundamental
Arp priority
Arrange windows
Arrangement name
Arrays not compatible
Articulator tools
As audio clips
As audio tracks
As instrument tracks
ASIO devices
ASIO properties
AsioExtra A
Aspect ratio
Assertion failed
Attack level
Attenuate Gibbs phenomenon
Attenuate ringing ripples
Audio bitrate
Audio Catalyst
Audio Catalyst VBR
Audio CD
Audio Clip
Audio clip
Audio clip sources
Audio clipboard
Audio clipboard file
Audio clipboard.wav
Audio clips
Audio codec
Audio DVD
Audio input
Audio input source
Audio logger
Audio Module Class
Audio output
Audio recording
Audio scripts
Audio source
AUDIO SRC
Audio track options
Audio track(s)
Audioactive P. Studio
AudioUnit (fx)
AudioUnit (gen)
Audition and select
Authentication failed
Authorization error.
Auto (sustain)
Auto clip
Auto close
Auto color group
Auto contrast text
Auto detect
Auto dump score
Auto fill
Auto locate
Auto loudness
Auto map
Auto map inputs
Auto map outputs
Auto name channels
Auto name clips
Auto normalize samples
Auto rename
Auto rename all
Auto resize
Auto save
Auto saved
Auto saved to
Auto scale
Auto scroll
Auto scrolling
Auto smoothing
Auto tempo
Auto visual mode
Auto-arrange modules
Auto-collapse layers
Auto-create audio clip
Autogun homepage
Automap extra outputs
Automap zone(s)
Automate event
Automatic chord detection
Automatic crossfades
Automation Clip
Automation clip
Automation clip sources
Automation clips
Automation editor
AUX SEND
Available presets
Average mode
Avoid prev.
Azure Mist
B +C
B -
Back up once
Background color
Background gradient
Background punches
Background rendering
Bad file number.
Bad protocol option.
Bad Request
Band distribution
Band gain multiplier
Band gain offset
BAND LIMIT
Band panning
BAND PASS
Band pass
Band phaser
Band stop
Band width
Bandlimited sawtooth
Bandlimited square
Bandlimited triangle
Bands - Distribution
Bands monitor input
BANK A
Bank A
BANK B
Bar override
Base key
Base Level
Base sample,
Base track select
Basic cross edits
Basic realtime resampling
Basic resampling
Basic single edits
Basic with limiter
Basic with limiter.flp
BasicInitialization failed.
Bass clef
BCD overflow
Beat shuffle
Beat sync
Beats per bar
BeatSlicer gap filling
BeforeRunning - end
BEHRINGER BCF2000
BEHRINGER CMD DC-1
BEHRINGER CMD LC-1
Behringer CMD LC-1
Below fundamental
Best mode
Bevel bottom
Bevel top
Big Band
Bin level
Bin width
Bin window shape
Bipolar LFO tension
Bipolar product
Bipolar sum
Bit addressing mode
Bit depth
Bit size
Black color
Black knobs offset
Black Notes
Black notes
Blanched Almond
Blend recorded notes
Blend recording
Block extraction
Block size
Blue average
Blue Violet
Blur - Default.fnv
Blur behind
Blur local mask
Booty Bass
Bottom Left
Bottom left
Bottom Right
Bottom right
Box hue picker
BPM SYNC
Breakdown 1
Breakdown 2
Bright Blue
Bright saw
Brighter saw
Browsed item,
Browser -
Browser and pickers
Browser contrast level
Browser enable indexing
Browser name
Browser pinned
Browser preview
Browser preview track
Browser refreshed
Browser search folders
Browser tab index
Browser view
Buffer fault.
Buffer length
Buffer length (DS)
Buffer offset
Build wavetables
Building block
Burly Wood
Burn loop
Burn MIDI to
Burn selected channels
Burn to pattern
Bus isn't mapped
Button caption
Button color
Button color (pressed)
Buy FL Studio
Buy plugin
Buy plugins
Buy this pack
Buy Upgrade
BW IIR
By color
By name
Bypass declicker
Bypass strum
C HcK
C0 - B6
C1 - B7
C2 - B8
C3 - B9
Cache, refresh data
Cadet Blue
Cakewalk A-Series
Call main
Camera angle
Camera zoom
Cancel all
Cancel all envelopes
Cancel plugin scan
Cap color (pressed)
Capture Height
Capture Width
Carrier Synthesizer
CCIT J.17
CD index
CD track
Center color
Center panned signal
Central European
Central Khmer
Ch 13
Chamber Music
Chan Filter BW
Chan Filter Freq
Chan Panning
Chan Volume
Channel Aftertouch
Channel aftertouch
Channel button width
Channel envelopes
Channel mode
Channel name
Channel name alignment
Channel panning
Channel pitch
Channel presets
Channel rack
Channel rack menu
Channel rack playback
Channel settings
Channel type
Channel Vocoder
Channel volume
Character map
Check again
Check limitations
Checking disk
Cherry phaser
Childs only
Chocolate Brown
Chocolate state variable
Chord name
Chord nudge
Chord progression
Chord repeats
Chorus blur
Chorus color
Chorus delay
Chorus depth
Chorus feedback
Chorus Send
Chorus speed
Christian Rap
CHROMA LINK
Circular hue picker
CLAP (fx)
CLAP (gen)
Classic band pass
Classic band stop
Classic high pass
Classic low pass
Classic peak meter
Classic Rock
Claw machine
Clean up (denoise)
Click reduction
Click-free (smooth) editing
Clip panning
Clip pitch (semitones)
Clip properties
Clip properties -
Clip volume
Clipboard files
Clipboard operation failed.
Clock resolution
Clock source
Cloning to destination
Collapse all layers
Collapse grouped tracks
Collapse layer
Collapse structure
Collect usage statistics
Color 1
Color 2
COLOR FREQ
COLOR GAIN
Color Map
Color map
Color Mapper
COLOR MODE
Color picker
Color selected
Color selector
Colorful channel controls
Colorful loop controls
Colorful mixer
Colorful waveforms
Combined plugins
Commit file
COMP OFF
Compact 2
Compact alt
Compact plugin list
Comparer -R
Comparer w
Compatible velocity
Compensation stretching
Compilation failed
Compiled ok
Compiled ok.
Compression level
Computer says no
Configure extra folders
Configure script
Connect to SoundCloud
Connect to YouTube
Connect with SoundCloud
Connect with YouTube
Connection failed
Connection timed out.
Connection was reset.
Connection was stopped.
Connection: keep-alive
Contact Support
Contact support
Content library
Content-Transfer-Encoding: 8bit
Content-Transfer-Encoding: binary
Continue anyway
Continue downloading
Continue syncing
Continue trial
Continuing more initialization
Continuous Controller
Continuous move
Continuous move pos
Continuous output
Continuous scrolling
Control Creator
Control creator
Control settings
Control Surface
Controller help
Controller plugins
Controller type
Converting sample format
Convolution reverb
COOL EDIT
Copying local
CoreAudio devices
Corner aspect
Corner size
Cornflower Blue
Count E
Count k4
Countdown before recording
CPU meter speed
Crash on loading
Crash processing
Crash resume
Crash set blocksize
Crash suspend
Crisp male
Crisp, clear male
Crop top
CROSS CUTOFF
CROSS TYPE
Crossfade (bleeding)
Crossfade amount
Crossfade selected
Crossfade tension edit
Crossfade with
Crude band pass
Crude band stop
Crude high pass
Crude low pass
Ctrl + I
Ctrl key modifier
Ctrl Number
Ctrl Type
Current Arrangement
Current clip source
Current engine location
Current instance
Current language
Current note
Current pattern
Current preset number
Current project
Current selection
Current snapshot
Current time
Current time (H:M:S:Frame)
Current time (H:M:S:MS)
Current time hint
Cursors PPI
Cursors scaling
Custom Grid Color
Custom image index
Custom mesh index
Custom painting
Custom shape 1
Custom shape 2
Custom size
CUTOFF FREQ
Cutoff freq
Cycle backward
Cycle down
Cycle layers
Cycle up
D+B D
DAMP ENV
Dance Hall
Dance move
Dark Blue
Dark Cyan
Dark Golden
Dark Gray
Dark Green
Dark Khaki
Dark Magenta
Dark Olive Green
Dark Orange
Dark Orchid
Dark Red
Dark Salmon
Dark Sea Green
Dark Slate Blue
Dark Slate Gray
Dark Turquoise
Dark Violet
Data folder
Data interchange format
Data usage permission
DAW IN
DAW OUT
DC offset removal
DCOM not installed
Deactivate all
Death Metal
Debug log
Decay per voice
Decay slope
Decay speed
Decimator Mix
Decimator Stp
Deck A
Deck B
Declick in
Declick out
Declicking mode
Deep chorus
Deep Pink
Deep Sky Blue
Default (macOS)
Default audio device
Default bus
Default child voice
Default color
Default editor
Default female voice
Default increment
Default male voice
Default ModX
Default ModY
Default panning
Default template
Default velocity
Deflate not available
Degrade horizontally
Degrade vertically
Delay audio output
DELAY MODE
DELAY MODEL
Delay Send
DELAY TIME
Delphi Component
Delphi Picture
Demo projects
Denoiser - default.fnv
Dephased saw
Dephased saw 2
Deselect all channels
Deselect all patterns
Deselect all tracks
Desktop (alternate)
Desktop (default)
Destination address required.
Destroy refresh thread
Destroy screen object
Detach all plugins
Device Independent Bitmap
Device name
Device not assigned
DIB image
DIB is invalid
DIB v5 image
DigiCert Inc1
Directory not empty
DirectSound devices
DirectSound properties
DirectWave Instrument rendering
DirectWave VSTi
DirectX (fx)
DirectX (gen)
DirectX 5
Disabled meters mode
Disarm selected track(s)
Discard changed shortcut
Discard changes
Discard lengths
Disconnect calls ExecuteOnGUIThread
Disk full
Disk operations
Dispatch midi message
Display all channels
Display bar
DISPLAY SECTION
Display settings
Display text
Display timed text
DJ-Tech Mix Free
DMX Device
Do nothing
Dock to
Dodger Blue
Doepfer DREHBANK
DoFail was set.
Dominant bebop
Don't ask
Don't change
Don't close
Don't copy
Don't distract me
Don't download
Don't group tracks
Don't loop
Don't remove selected
Don't report
Don't restart now
Don't save
Don't show
Don't show again
Don't slice
Don't stretch
Don't unlock
Don't warn me
Done initializing things
Double alt
Double cone
Double curve
Double curve 2
Double curve 3
Double outline
Double saw
Double stroke
Down keyboard key
Download all
Download and install
Download broken
Download cancelled
Download data broken
Download failed
Download finished
Download FLEX pack
Download free demo
Download latest beta
Download now
Download pack
Download Placeholder
Download project backup
Download updated version
Download URLs retrieved
DPI aware
Drab olive
Dragged sample
Draw rectangle
Draw text
Driver incompatible
Driver restricted
Drum and bass
Drum machine
Drum mode
Drum pads
Drum room
Drum Solo
Drum(loop) stretch
Drum(loop) stretcher
Drunken male
Dry Amount
Dry guitar
DRY MIX
DTD Declaration
Dual channel (Stereo)
Dual keyboard
Dual view
Duration (ms)
DW Player
DX not found
E +E(
E +HL
E +X H
E 9u
E A)E
E HcP
E(+E +
E(+E H
E(+E Hc
E(9X t5
E(H+p H
E-mail address
Early double stroke
Early flam
Earth Red
Easy Listening
Echo - classic
Echo feedback
Echo grains
Edge cases
Edison (feature)
Edison Declipper Windows
EDITOR OPTIONS
Editor options
EEAllAudioClipsSepMenu 2
EEExpandAllGroupedTracksMenu 3
EESelAudioClipsE2GenericMenu 0
Effect into channel
Elderly woman
Electric guitar
Electrix Tweaker
Empty brackets ()
Empty project
Empty Tag
EMU volume envelope
Encode FLAC
Encode M4A
Encode media
Encode MIDI
Encode MP3
Encode OGG
Encode WAV
Encoder nicht erkannt
End Tag
End time (H:M:S:Frame)
End time (H:M:S:MS)
Endless controller
Endless knob
English (UK)
English (US)
Enhanced frequency
Enhanced metafile image
Enhanced Metafiles
Enhanced time
Enter password
Enter preset name
Entertain me
Enttec OpenDMX USB
ENV 1
ENV 2
Env Amt
ENV MOD
ENVELOPE 2
Envelope attack
Envelope editor
Envelope Follower
Envelope Level
Envelope preset name
Envelope release
EQ - default.fnv
EQ Hi
EQ Lo
Erase rectangle
Event bus
Event persistence
Events - LFO
Exception on open
Exclusive random
Expand all layers
Expand folders
Explore script folder
ExportAllPLTracksMenu (
External clock sync
External input only
External sync
External tools
External window
Extra color
Extra data
Extra large
Extra small
Extreme HQ
F3 - E10
Factor coarse
Factory Info
Factory Presets
Fade in
Fade in curve
Fade out
Fade out curve
Fade set manually
Fade stereo
Fail reaction
Fake stereo
Fancy animations
Fancy mode
Fast flanger
Fast Fred
Fast fred
Fast Fusion
Fast idle
Fast LP
Fast sample preview
Fast talking voice
Fat - detuned
Fat mode
FB FILTER
FEEDB. HP
FEEDBACK DISTORTION
Female speech
Field already defined
File access denied
File date
File is empty
File name(s)
File not found
File saved
File saving failed
File size
FILTERING SECTION
Fin low pass
Final output
Fine Pitch
Fine pitch
Fine Tune
Fine tune
Fingertip control
Finished (call main)
Finished (Create)
Finished (CreateAndInitView)
Finished (early)
Finished (find plugins)
Finished (LoadCubasePreset)
Finished (no effect)
Finished (RESTORE)
Finished (SAVE)
Finished (SaveCubasePreset)
Finished (TVST3Editor.Create)
Finished (TVST3Plugin.Create)
Finished (TVST3Plugin.Destroy)
Finished (TVST3Plugin.LoadPlugin)
Finished (VSTPlugin.Destroy)
Finished early 1
Finished early 2
Finished early 3
Finished early 4
Finished early 5
Fire Brick
Fire Edition
Fire edition
First half
Fist mode
Fix generate seed
Fix unexpected levels
Fixed bass
Fixed color
FL (fx)
FL (gen)
FL 64
FL CLOUD
FL Cloud
FL Cloud Plugins
FL CLOUD Sounds
FL Help
FL Help index
FL KEYS
FL Keys
FL STUDIO
FL Studio
FL Studio (
FL Studio +
FL Studio 10
FL Studio 11
FL Studio 12
FL Studio 20
FL Studio 2026
FL Studio 21
FL Studio 24
FL Studio 25
FL Studio 26.1
FL Studio 4.1.2
FL Studio 8
FL Studio 9
FL Studio ASIO
FL Studio automation
FL Studio Browser
FL Studio Closed
FL Studio DXi
FL Studio engine
FL STUDIO FIRE
FL Studio folder
FL Studio Languages
FL Studio license
FL Studio Mobile
FL Studio News
FL Studio Opened
FL Studio Remote
FL Studio ReWire
FL Studio Themes
FL Studio transport
FL Studio v
FL Studio VSTi
FL Studio webpass
FL Window settings
Fl1 Cutoff
Fl1 Resonance
Fl1 Shape
Fl2 Cutoff
Fl2 Resonance
Fl2 Shape
FLAC bit depth
FLAC compression
FLAC compression level
FLAC export
Flash warning
Flat band pass
Flat buttons
Flat hat
Flat pitch contour
Flats -
Flip horizontally
Flip state
Flip values
Flip vertically
Flip Y
Floating point overflow
Floating point underflow
Floral White
FLRemote controller
FLS Info
Flush all
FM SYNTH
FM Synthesizer
Focus background
Focus on click
Focus text color
For easier recording
Forbidden filename
Force boolean
Force refreshes
Foreground color
Forest Green
FORMANT CORRECTION
Formant preservation
Forward all
Found 1 slice
FPC Center
FPC Left
Frame rate
Free 1
Free 1:1
Free 1:2
Free 1:3
Free 1:4
Free 1:5
Free 1:6
Free 1:7
Free 1:8
Free 2
Free 2:1
Free 2:2
Free 2:3
Free 2:4
Free 2:5
Free 2:6
Free 2:7
Free 2:8
Free 3
Free 3:1
Free 3:2
Free 3:3
Free 3:4
Free 3:5
Free 3:6
Free 3:7
Free 3:8
FREE FOR CUSTOMERS
Free for customers
Free jog mode
Freq scale
FREQ. RANGE
FREQ. SHIFTER
FREQ. SHIFTER HQ
Frequency precision
Frequency Shifter
Frequency Splitter
From song start
From track start
Frontier Design AlphaTrack
Frontier Design TranzPort
Frozen LFO
Fruit Kick
Fruity Balance
Fruity Bass Boost
Fruity Big Clock
Fruity Blood Overdrive
Fruity Center
Fruity Chorus
Fruity Compressor
Fruity Convolver
Fruity Dance
Fruity dB Meter
FRUITY DELAY
Fruity Delay
Fruity Delay 2
Fruity Delay 3
Fruity Delay Bank
Fruity DrumSynth Live
Fruity DX10
Fruity Edition
Fruity effect
Fruity Envelope Controller
Fruity Fast Dist
Fruity Fast LP
Fruity Filter
Fruity Fire
Fruity Flanger
Fruity Flangus
Fruity fonts
Fruity Formula Controller
Fruity Free Filter
Fruity generator
Fruity Granulizer
Fruity HTML NoteBook
Fruity Keyboard Controller
Fruity Limiter
Fruity Love Philter
Fruity LSD
Fruity microfont
Fruity Multiband Compressor
Fruity Mute 2
Fruity NoteBook
Fruity NoteBook 2
Fruity PanOMatic
Fruity Parametric EQ
Fruity Peak Controller
Fruity Phase Inverter
Fruity Phaser
Fruity Reeverb
Fruity Reeverb 2
Fruity Scratcher
Fruity Send
Fruity Slicer
Fruity Slicer 2
Fruity Soft Clipper
Fruity Soundfont Player
Fruity Spectroman
Fruity Squeeze
Fruity Stereo Enhancer
Fruity Stereo Shaper
Fruity Video Player
Fruity Vocoder
Fruity Voltage Controller
Fruity WaveShaper
Fruity Wrapper
Fruity Wrapper -
Fruity X-Y Controller
Fruity X-Y-Z Controller
Full blur
Full name
Full porta
Full sample preview
Full song
Full window
FX CHORUS
FX DELAY
FX DRIVE A
FX DRIVE B
FX ORDER
FX REVERB
Gain edit
Gain normalization
Gate noise
GATE THRESH
GATE TIME
Gear color
Generate noise
Generate random cloud
Generate random preset
Generate steps
Generated new steps
Generator muting
Generic (bleeding)
Gentle 4
Gentle 6
Gentle 8
Geode by NSC
Ghost Producer
Ghost White
Giant male
GIBBS OFF
Glass effect
GLIDE MODE
Global EQ
Global LFO amount
Global LFO phase
Global LFO speed
Global link settings
GLOBAL PITCH
GLOBAL SPEED
Global transport
Gloss color
Glow color
Glowing lights
Glue same notes
Glyph color
GM CCs.txt
GM Drums.txt
Go back
Go to account
Go to website
Gold Fest
Golden Sky
Gopher Control
Gopher opened
Got it
Gothic Rock
GR. SPACING
Gradient color
Gradient colors
Gradient editor
Granite Gray
Graph editor
GRAPHIC EQ
Graphics overloaded
Green average
Green Yellow
Grid Color
Grid color
Grid Contrast
Grid contrast
Gross Beat
Grouped tracks
GUI display
GUI input
H zoom
H+C H
H+P M3
Half cosine
Half height row
Half sine
Half viewport height
Half viewport width
Half width column
Handle color
Handle color (pressed)
Handled (first chance)
Handled (last chance)
Hard low threshold
Hard Rock
Hardware synthesizer
HARM CLIP
HARM PROT
HARM RAND
Harm up
Harm up +1
Harmonic bins
Harmonic blur amount
Harmonic clipping
Harmonic clipping threshold
Harmonic minor
Harmonic phase
Harmonic prism
Harmonic protection
Harmonic randomness
Harmonic unison pitch
Harmonizer distribution
Harmonizer local mask
Harmonizer mix
Harmonizer width
HcC Mc
HcF H
HcG H
HcI H
HcI Hi
HcK H
HcM H
HcQ Hi
HcQ Lc
HcU LcCtI
Heatmap position
Held index mapping
HF Exciter
HI CUT
HI DAMP
Hidden clips
Hiding collapsed tracks
High (melodic)
HIGH BAND
High band
HIGH CUT
High Cut
HIGH DAMPING
High Damping
High Hat
High key
HIGH MID
High Note
HIGH PASS
High pass
High precision
High quality rendering
High shelf
High threshold
High velocity
High Visibility
High visibility
Hill low pass
Hint bar
Hint location
Hints and warnings
Hints shown here
Hiri Motu
Hold  motion
Hold  stop
Hold time
Honey Dew
Horizontal fader
Horizontal meters mode
Horizontal scrolling speed
Horizontal zoom
Host is down.
Host is unreachable.
Host not found.
Hot Pink
HQ ENVELOPES
HQ envelopes
HQ RENDERING
HQ rendering
HQ resampling
HQ WaveShaper
HTML Format
Hungarian minor
Hybrid low pass
Hyper Chorus
Hz scale (bipolar)
I'm in
I3q H1
IcE H
Icons only
Ignore host selection
Ignore own selection
Ignore silent takes
Ignore tempo changes
II PAUSE
IL Product Versions
IL shared data
Image content
Image fine speed
Image formant mutation
Image formant shift
Image Line Envelope
Image Line NV
Image Line NV0
Image Line NV1
Image Line Shop
Image saved
Image selected column
IMAGE SRC
Image time offset
Image-Line news
Image-Line Remote
Image-Line Shop
ImageLine Reference
Inactive color
Increased frequency precision
Increment pattern length
Increment value
Incremental scrolling
Incremental search
Infinite hold
Inherit bandwidth
Init core paths
Init DWM
Init encoding
Init launchmap pages
Init module
Init screen object
Init things
Initialize audio outputs
Initialize fonts
Initialize mastering
Initialize MIDI
Initialize Python
Initialize real-time processing
Initialized controls
Input device type
Input disabled
INPUT FILTER
INPUT HP
Input mapping
Input name
Input port
Input stereo separation
Input volume
Inserted event
Instrument track options
Instrumental Pop
Instrumental Rock
Interface not supported
Internal controller
Internal controllers
Internal error
Internal FFT precision
Internet error
Internet Explorer
Interpolate (white only)
Interrupted system call.
Into workspace
INV FEEDBACK
Inv grid
Inv Saw
INV WET
Inv. stereo
Invert grid
Invert pencil buttons
INVERT WET
Inverted increment
Inverted legato
Inverted log scale
Inverted porta
Is blanked
Is device assigned
Is metronome enabled
Is precount enabled
Is Unblank
IS:off, MS:off
IS:off, MS:on
IS:on, MS:off
IS:on, MS:on
IsEmpty Dh
Item not found
Item TChanList.TChanListEventListener
Item TRealtimeProcessorImpl.TSetParam
Item TSummaryMsgBoxImpl.TMsgBoxParams
Its settings
ITU-R BS.1770
Jack plug
Jap in sen
Jet Set Willy
Jog 1
Jog 2
Jog marker jump
Jog marker selection
Joint stereo
JPEG Image
Jump to pattern
Jumped to
Just chop chords
K(H+K H+
KBD TRACK
Keep at velocity
Keep automations
Keep display active
Keep every 3
Keep every 4
Keep every 5
Keep focus
Keep in front
Keep it sober
Keep on disk
KEEP PITCH
Keep-Alive: 300
Keith McMillen QuNeo
Kepler Exo
Key Aftertouch
KEY FLW
Key frame spacing
KEY M
Key Mapper
Key size
Key step
KEY TO
Key to pitch
Keyboard 1
Keyboard 2
Keyboard height
Keyboard mapping
Keyboard options
Keyboard Splitter
Keyboard style
Keyboard tracking
Keyfax Phat.Boy
Keys 6
Keys per zone
Keys x
Kick drum
Kill all
Kill same
KIM TESTING
Kit name
Knob accel off
Knob accel on
Knob tweaks (Volume)
Korg KONTROL49
Korg microKONTROL
Korg MS-20 Controller
Korg nanoKEY
Korg nanoKONTROL
Korg nanoKONTROL2
Korg nanoPAD
Korg padKONTROL
L + R
L Studio
L+k M
L-R ENCODING
Label already defined
Language change
Language Changed
Language changed
Language support update
Language: zh-Hans
Large grid-slicing
Large hall
Large male
Last 1
Last 10 minutes
Last 2
Last 2 minutes
Last 20 minutes
Last 30 minutes
Last 5 minutes
Last BRR date
Last end
Last half
Last hit
Last reset
Last SER
Last start
Last SUR
Last tweaked
Last used project
Latency (ms)
Latency (samples)
Latin 9
Launch at startup
Launch audio editor
Launch content library
Launch with file
Launch with path
LAUNCHKEY MK2
Lavender Blush
Lawn Green
Layer both decks
Layer homonymous regions
Layer homophonic regions
Layer name
Layers arrangement editor
Layout File
Layout Piano
Layout select
LcE M3
LcK L
Leaky low pass
Leap Motion
Leave duration
Leave duration unchanged
Leave end time
Leave note duration
Leave remainder
LEFT + RIGHT
Left channel
Left hand
Left keyboard key
Left voltage
Leg low pass
Legacy scaling
Lemon Lime
Length (beats)
Length (s)
Length: 1 bar
Lengths per loop
Less bass
Less mid
Less smooth
Less warm
Let's get started
Level meter editor
Level offsets
Level Scaler
Level scaling
Levels adjustment
LF Exciter
LFO 1
Lfo 1
LFO 1 FREQ
LFO 1 WAVE
LFO 2
Lfo 2
LFO 2 FREQ
LFO 2 WAVE
LFO 3
LFO 3 FREQ
LFO 3 WAVE
LFO Level
LFO PULL
LFO Pulse Width
LFO RATE
LFO SCALE
LFO Skew
LFO Speed
LFO Tension
License has expired
License plate
License will expire
Light Blue
Light Coral
Light Cyan
Light Goldenrod Yellow
Light Gray
Light Green
Light magenta
Light mode
Light Peach
Light Pink
Light Salmon
Light Sea Green
Light sensor
Light Sky Blue
Light Slate Gray
Light Steel Blue
Light Yellow
Lime Green
Lime high pass
Lime low pass
Limit CPU
Limit CPU usage
Line color
Line Color (right)
LINEAR PHASE
Lines between tracks
Linked layout
LINKED TO
LINN CONGA
List Z
Live display zone
Live selection
Live selection start
Livid BASE
Livid Base
Livid Block
Livid CNTRL-R
Livid OhmRGB
LMH DEL
LMH MIX
Loading plugin list
Loading project: version
Loading tags finished.
LOCAL EQ
Local EQ
Locale descriptor
Localization problem
Locate data
Locate download
Locate file
Locate sample
Locate this pack
Locating file
Lock all zones
Lock audio data
Lock display zone
Lock harmonics
Lock part
Lock to content
Log scale
Logical Performed DropEffect
Look Gap Range
Looping bias
Lossy normalize
Lotsa free controls
Low (drums)
LOW BAND
Low band
LOW BASS
LOW CUT
Low Cut
Low key
Low Lifter
LOW MID
Low Note
LOW PASS
Low pass
Low pitched male
Low shelf
Low velocity
Lowpass filter
LP FIR
LP Random
Lucida Console
Luminance average
M +AL
M 2nd
M 3rd
M 6th
M 7th
M 9AduVH
M :A
M :AS
M D+A
M H9A
M H9H
M HcI
M HcI0H+
M HcItH
M(HcI H
M-Audio Oxygen 25
M0HcA H
Mackie Control Universal
Magellan MSWHEEL
Magic lasso
Magnifying glass
Main automation
Main GUI scaling
MAIN OUT
Main Output
Main PPI
Main settings
Major bebop
Major penta
Make all loop
Make bridged
Make it pretty
Make unique
Male speech
Manage cloud accounts
Manage messages
Manage plugins
Manage shortcuts
Manage subscription
Manage tabs
Manage warning messages
Manage your license
Mango low pass
Manual mapping
Mapping formula
March  stay
March  stop
March  wrap
Marker base name
Marker count
Marker jump
Marker jump mode
Marker loop
Marker name
Marker pause
Marker sel
Marker select mode
Marker settings
Marker skip
MASTER FILTER
MASTER FREQ
Master pitch (Hz)
Mastering failed
Mastering loudness
Mastering loudness preset
Mastering reference A
Mastering reference B
Mastering tuning A
Mastering tuning B
MAX DEPTH
Max length
Max per chord
Max release (s)
Max res
Max Velocity
Maximize Piano roll
Maximize Play list
Maximum height
Maximum undo levels
MB Compressor
Medium Aquamarine
Medium Blue
Medium grid-slicing
Medium Orchid
Medium Purple
Medium Red Violet
Medium Sea Green
Medium Slate Blue
Medium Spring Green
Medium Turquoise
Mellow female
Mellow male
Melodic minor
Melodic score
Memory program menu
Menu inserted twice
Menu item click
Menu transparency
Message too long.
Metallic Violet
Meter background
Metronome number
Metronome track
Mh+q )
Microsoft MIDI Mapper
Microsoft Sans Serif
Microsoft Win32s
Microsoft Windows 2000
Microsoft Windows 7
Microsoft Windows 95
Microsoft Windows 98
Microsoft Windows 98SE
Microsoft Windows NT
Microsoft Windows Vista
Microsoft Windows XP
MID BAND
Mid band
Middle note
MIDI CC
MIDI channel
MIDI channel through
MIDI clipboard
MIDI clock
MIDI controller
MIDI detection
MIDI from clipboard
MIDI hardware port
MIDI import
MIDI import window
MIDI IN
Midi Loop
MIDI MODULATION MATRIX
MIDI Note
Midi Note
Midi note
MIDI options
MIDI OUT
MIDI Out
MIDI out
MIDI patch
MIDI port
Midi rec session
MIDI RPN
MIDI scripts
MIME-Version: 1.0
MIN DECAY
MIN DEPTH
MIN TIMES
Mini playlist preview
Mini view
Minimum height
Minor penta
Mint Cream
Mirror horizontally
Misty Rose
Mix down
MIX LEVEL
Mix level
MIX VOL
Mix volume
Mixdown amp envelopes
Mixed view
Mixer FX
MIXER MATRIX
Mixer presets
Mixer States
Mixer track
Mixer track number
Mixer track routing
MM OUT
Mobile Data
Mobile rack data
MOD AMP
Mod Amt P1-1
Mod Amt P1-2
Mod Amt P1-3
Mod Amt P1-4
Mod Amt P2-1
Mod Amt P2-2
Mod Amt P2-3
Mod Amt P2-4
Mod Amt P3-1
Mod Amt P3-2
Mod Amt P3-3
Mod Amt P3-4
Mod Amt P4-1
Mod Amt P4-2
Mod Amt P4-3
Mod Amt P4-4
MOD FREQ
MOD MATRIX
MOD SOURCE
Mod Val 1
Mod Val 2
Mod Val 3
Mod Val 4
Mod Wheel
Mod wheel (Wrapper)
MOD X
Mod X
MOD Y
Mod Y
MODULATION 1
MODULATION 2
Modulation X
Modulation X mapping
Modulation X-mapped
Modulation Y
Modulation Y mapping
Modulation Y-mapped
Modulation Z
Modulation Z mapping
Modulator noise level
Modulator pass-through
Modulator pitch shift
Module Prototype
Module's input level
Module's output level
Monitor input
Mono (left only)
Mono (merged)
Mono (right only)
Mono - Merged
Mono channels
MONO INPUT
MONO KEY
Monolithic file
Monolithic mode
More bass
More cowbell
More info
More information
More initialization
More mid
More options
More plugins
MORPH TO
MORPHING EQUALIZER
MOUSE MAT
Moved to
Movie score
MP3 bit rate
MP3 bitrate
Mp3 Compressor
MP3 export
MS Sans Serif
MTC 24 fps
MTC 25 fps
MTC offset
MTC Quarter Frame
MulDiv test
MULTI BANK
Multi mode
Multi-button mouse
Multi-touch monitor
MULTIBAND DELAY
Multiband Delay
Multichannel waveform view
Multilink to controllers
Multiple outputs
Multiple patterns
Multiple selection
Multithreaded generator processing
Multithreaded mixer processing
Multitimbral mode
My account
My Magenta
My projects
N H+N
N116 4
Name and location
Name for channels
Name for patterns
Names at top
National Folk
Native American
Native Instruments MASCHINE
Natural pitch contour
Natural scale
Natural weighting
Nature sounds
Navigate browser
Navigate browser menu
Navigate browser tabs
Neap. minor
Nebula picker
Nektar Impact series
Nektar Panorama P4
Nerdish male
Network is down.
Never restart
Newstuff shown
Next memory program
Next window
Next zoom area
NO WAVE
Noise Band
NOISE BP
NOISE GATE
NOISE LEVEL
Noise profile acquired
Noise profile required
Non-Diatonic notes
Non-Registered Parameter
Non-registered parameter
None (0 V)
Normal voicing
North Ndebele
Northern Sami
Norwegian Bokm
Norwegian Nynorsk
Not Connected
Not enough space
Not found
Not handled
Not implemented
Not supported
Not Used
Note adjustments
Note color presets
Note colors
Note cutoff frequency
Note end time
Note input
Note Key
Note linking
Note markers
Note mode
Note modulation X
Note modulation Y
Note name presets
Note names
Note Off
Note off value
Note On
Note parameter
Note pitch
Note preview
Note properties
Note properties -
Note Range
Note release
Note resonance
NOTE SELECT
Note start time
Note Velocity
Note velocity
Notes and automation
Nothing to chop
Nothing to clone
Nothing to convert
Nothing to undo
Novation Launch Control
Novation Launchkey
Novation Launchkey (keyboard)
Novation Launchkey (performance)
Novation Launchkey MK2
Novation Launchpad
Novation Launchpad MK2
Novation Launchpad Pro
Novation Launchpad S
Novation ReMOTE SL
NR. STAGES
Num programs
Numark Orbit
Object not Initialized
Oct up
Oct up +1
Oct up +2
Octave scale (bipolar)
Octave wrap
OEM text
Off (polyphonic)
Off color
OGG bit rate
OGG bitrate
Ogg Vorbis
Ogg Vorbis export
Ogun homepage
OiCCPPhotoshop ICC profile
Old Lace
Old projects
Old woman
Omni -
Omni channel
Omni preview
On (FLAC Compressed)
On (Uncompressed)
On close
On color
On demand
On desktop
On input
On over
On play
On press
One note preview
One octave down
One octave up
One plugin
One semitone down
One semitone up
One shot
One-click audio recording
OnEdited P
OnFindComponentClass wM
OnGetLibraryPath not set
OnKeyNotify +
OnKeyNotify U
OnKeyNotify Z
ONLINE CONTENT
Online content
Online content library
Online help
Only me
Only one
Only when recording
Only with selection
OpenGL 2.0
OpenGL 4
Operation not supported
Operation would block.
Operator 1
Operator 2
Operator 3
Operator 4
Operator 5
Operator 6
Optimistic lock failed
Optimize for
Optional Features
Optional features
Orange Red
Order form
Original key
Original length
OSC 1
OSC 1+2
OSC 2
OSC 3
Osc2 freq
Oscillator 2 frequency
Oscilloscope ruler
OUT 2
OUT 3
OUT 4
OUT GAIN
Outline color
Output device
OUTPUT FILTER
Output format
Output mapping
Output meter
Output name
Output noise only
Output port
Output processing
Output stereo separation
Output summary
Output volume
Overview mode
Overwrite all
Overwrite file
Overwrite files
P HcE,H
P HcETH
P LcEh
P. DELAY
Pad 1
Pad brightness
Pad saturation
Page lock
Page selector
Pale Goldenrod
Pale Green
Pale Turquoise
Pale Violet Red
Palette data
Palette editor
Panning law
Papaya Whip
Param 1
Param 2
Param 3
Param 4
Param 5
Param 6
Parameter count mismatch
Parameter not set
Parameter tuning
Parameters randomized
Parent folder
Pat   Mix   Chan
Patch bank
Patch bank.flp
PATCH NAME
Patch selector
Patcher v0
Patcherize all
Path is empty
Pattern clip
Pattern clip sources
Pattern controls
Pattern group name
Pattern length
Pattern selector
PB Rng
PC ARM
Peach Puff
PEAK FREQ
PEAK GAIN
Peak Level
PEAK Q
Peaks Left
Peaks Right
Peavey StudioMix
Pen data
Pending changes
Pending order
Per part voice
Per unison voice
Per voice
Perfect all regions
Perform full refresh
Perform quick quantize
Performance clip progress
Performance data
Performance mode
Performance track progress
PERFORMANCE vs QUALITY
Performed DropEffect
PHASE OFFSET
PHASE OFS
PHASE RAND
Phase rotation
Phaser Freq
Phaser mask
Phaser Mix
Phaser mix
Phaser offset
Phaser shape
Phaser to frequency
Phaser width
Piano keyboard preview
Piano roll
Piano roll -
Piano roll script
Piano roll scripts
Pickerpanel pattern 1
Pickup (takeover mode)
Ping pong
Ping pong loop
PITCH ANALYSIS
Pitch Bend
Pitch coarse
Pitch scale
Pitch scaling factor
PITCH SHIFT
Pitch Shifter
Pitch slide
Pitch Tune
Pitch up beat
Pitch vibrato depth
Pivot slope
Place loop
Plain text
Playback speed
Playback tracking
Player Limitation
Playlist insert track
Playlist markers
Playlist play
Playlist to pattern
Playlist track
Please choose another
Please try again
Please try again.
Please wait.
PList 8
Pluck amount
Pluck shape
Plugin Compatibility Class
Plugin database
Plugin databases
Plugin manager opened
Plugin name
Plugin not found
Plugin not supported
Plugin not valid
Plugin output properties
Plugin performance monitor
Plugin picker
Plugin presets
Plugin search paths
Plugin version
Pneed dictionary
Pointer dent size
Pointer on cap
Poly aftertouch
Pop-ups scaling
Popups PPI
Porn Groove
Portable Network Graphics
Portamento octave wrap
Post effects
Post EQ
POST FILTER
POST GAIN
Post Gain
Post shift
Post track
POT-Creation-Date: 2022-08-17 11:41
Potentially slow operation
Powder Blue
Power Ballad
Powered by DirectX
PPI flags
PPM style meter
PRE AMP
PRE BAND
Pre effects
Pre shift
Precise time indicator
Precomputed effects
Precount time
PREDELAY FB
Preferred DropEffect
Prepare time envelope
Preparing Download
Preserve end
Preserve start times
Preset colors
Preset filename
Preset name
Preset number
Pressed color
Pressed size
Previous memory program
Printing in progress
Prism amount
Privileged instruction
Pro (Trial)
Pro Audio
Pro default
Pro transient
Process HF
Process LF
Process map item
Process rec event
Processing failed
Processing layers
Processing quality
Processing zip file
Producer Edition
Product information
Program Language
Program language
Program Lfo 1
Program Lfo 2
Progression presets
Progressive Rock
Project backups
Project bones
Project data
Project folder
Project name
Project picker
Project size warning
Project tempo
Project type
Project-Id-Version: FL Studio
Projects bones
Property is read-only
Protocol not supported.
Psychedelic Rock
Pulse1 .
Punk Rock
Quad front
Quad rear
Quarter cosine
Quarter sine
Quarter viewport height
Quarter viewport width
Quick chop
Quick legato
Quick map to
Quick quantize
Quick rename
Quick zoom 1
Quick zoom 2
Quick zoom 3
Quit called
Rail color
Rail ring color
Random (avoid previous)
Random color
Random curve
Random mapping
Random portamento
Random presence
Random seed
Random Value
RATE FREQ1
RATE FREQ2
Razer Chroma
Read audio options
Read more options
Read various options
Reader not available
ReadIniFile - end
Realign events
Realtime stretching
REC IN
RecChan.Defines 5
RecChan.Defines B
Receive notes from
Recent controls
Recent files
Recent files.scr
Recent projects
Recompute tempo map
Recording has stopped.
Recording options dialog
Recording precount
Red average
Redeem code
Reducer Bits
Reducer Mix
Reference A
Reference B
Reflection bottom
Reflection top
Refund policy
Regenerate current chord
REGION SETTINGS
Regional note names
Register code online
Registered Parameter
Registered parameter
Registration details
REL 2
Release after
Release all
Release map item
Release slope
Release Velocity
Release velocity
Release velocity mapping
Reload all
Reload effect
Reload plugin
Reload script
Relocate events
Remember my choice
Remember seek time
Remember tab size
Remember tab sizes
Remind me
Remix a song
Remote control
Remote control settings
Rendering not available
RenderWAV -
Renew license
Renewal date
Renewing license
Repair optional features
Repeat midi event
Repeat once
Repeat slices twice
Repeat twice
Report Crash
Report latency
Request is empty
Request redirect failed
Reread structure
Resampling quality
Resave all presets
Resize all tracks
Resize now
Resonance shape 1
Resonance shape 2
Resource missing for
Respect recording filter
Restart the program.
Restart without saving
Resulting delays visualization
Return value expected
REVERB MIX
Reverb Send
Rewind song
ReWire device
Rhythmic Soul
Rich Text Format
Riff audio data
Riff machine
Right channel
Right keyboard key
RIGHT TAP
Right voltage
RING 1x2
RING MOD
Ring Mode Mix
Ring Mode Rate
RM FREQ
RM MIX
RM mix
RM OUT
RMS measurement
Rock  Roll
Room FX
ROOM SIZE
Room Size
Root key
Root note
Root note color
Root notes
Root offset
RootNodeCount A
Rosy Brown
Rotate down
Rotate left
Rotate right
Rotate up
Round to square
Rounded PPI
Routing cables
Royal Blue
Ruler Big Steps
Ruler Small Steps
Run counter
Run script
S H+S(A
S lm
Saddle Brown
Safe mode
Safe overloads
Same corners
SAME DECK
Sample history
Sample next voice
Sample Pos
Sample properties
Sample rate
Sample rate (Hz)
Sample Start
Sampling options
Sampling temperature
Sandy Brown
Saturation curve
Saturation mix
Saved to
Scaling250Menu -
Scope Long
Scope Mode
Scope Stereo
Scratch (envelope)
Script 1
Script 2
Script 3
Script 4
Script debugging mode
Script editor
Script message
Script output
Scroll to
Scroll to position
Scroll window
Scroller above
Scroller color
Sea Green
Sea Shell
Sedge hat
Segoe UI
Sel (beats)
Sel from
Selected color
Selected plane
Selected planes
Selected Program
Selection active
Selection end
Selection start
Self-extracting Archive
Semitone steps
Sends Output
Sent to clipboard
Sequence length
Ser track volume
Server side error
Setup screen properties
Several plugins
Sexy female
Shadow size
Shared data
Sharp low threshold
Shell IDList Array
ShellItem -Y
Shift +
Shift down
Shift key modifier
Shift left
Shift right
Shift up
Short name
ShowGopher action
Shuffling pattern
Sichuan Yi
SIDECHAIN INPUT
Sign in
Signature Bundle
Silent Startup
Silent startup
Simple 2D particlesystem
Simpler Grid
Since 2000
Singing mode
Single channel (Mono)
Single curve
Single curve 2
Single curve 3
Single cycle
Single device
Single pattern
Sinn7 Kontrolux
SiS SiS SiS
Size aspect
Size to fit
Skip all
Skip this plugin
Sky Blue
Slate Blue
Slate Grey
Slave device layout
Slave tempo
Sliced audio
Sliced beats
Sliced loop
Slide to
Slow Jam
Slow Rock
Slowing pace
Small Fonts
Small grid-slicing
Small studio
Smart disable
Smart search for
Smooth (bleeding)
Smooth curve
Smooth fade
Smooth pulse
Smooth saw
Smooth sawtooth
Smooth stairs
Smooth up
Smooth up (hard)
Smooth up (soft)
Smoothing attack compensation
Smoothing factor
SMOOTHING TIME
SMP RATE
SMP START
SMPTE (23.976 fps
SMPTE (24 fps
SMPTE (25 fps
SMPTE (29.97 fps
SMPTE (30 fps
SMPTE (Auto)
SMPTE 120 fps
SMPTE 23.976 fps
SMPTE 24 fps
SMPTE 25 fps
SMPTE 29.97 (non-drop)
SMPTE 30 fps
SMPTE 48 fps
SMPTE 50 fps
SMPTE 59.94 (non-drop)
SMPTE 60 fps
SMPTE 72 fps
Snake low pass
Snare drum
SOFT CLIP OUTPUT
Soft low threshold
Software synthesizer
Solid (more contrast)
Solved, reload project
Song (
Song data
Song info
Song jump
Song marker jump
Song Position Pointer
Song selection
Song time-synced
Song title
Sound Clip
SOUNDFONT PLAYER
Soundfont Player
Source arrangement
South Ndebele
Southern Rock
Southern Sotho
Speaker Settings
Spectral mask
Spectrogram band count
Spectrum gradient
Spectrum gradients
Spectrum view
Speech properties
Splitting your track
Spout (
SPREAD EVEN
Spring Green
Square Cut
Square Full
Square Light
Square Medium
Square Rounded
Square wave synthesizer
Squaretooth 2
SSSelAudioClipsE2SpeechMenu 1
ST. PHASE
Stack overflow
Standard jog mode
Standard MIDI File
Start creating
Start new project
START OFFSET
Start on input
Start over
START PHASE
Start playback
Start point
Start recording
Start recording immediately
Start selection
Start Tag
Start time
Start time (H:M:S:Frame)
Start time (H:M:S:MS)
Start various workers
Started as administrator
Starting at
Starting plugin scan
Startup project
Status: error
Status: off
Status: ready
Steel Blue
Steep 4
Steep 6
Steep 8
Stem extractor Windows
Step 1.
Step count
Step edit
Step editing mode
Step pitch
Step properties
Step seq mode
Step sequence
Step sequencer tutorial
STEP TYPE
Steps per beat
StepSeq 808 channels
Stereo channel pairs
STEREO DELAY
Stereo delay
STEREO ENHANCER
Stereo left
Stereo presence
Stereo right
STEREO SEP
Stereo sep
STEREO SEP IN
STEREO SEP OUT
Stereo separation
Stereo signals
Stock note lengths
Storage created
Storage space
Store plugin IDs
Store presets
Storing plugin presets
Stream read error
Stream write error
Streams the same
Stretch Pro
Stretch to compensate
Stretch x
Stutter (fourth)
Stutter (half)
Stutter 1
Stutter 2
Stutter 3
Sub Categories
SUB PHASE
Subscription status
Subtractive denoising
Surface Pro
Surface Pro hack
Surround flanger
Surround FX
Sustain level
SVF LP
SVF LPx2
Swap channels
Swap panels
Swap stereo
Swap with
SWEEP DOWN
SWEEP FREQ.
SWEEP UP
Swept Tone
Swing mix
SwingMix75Menu '
Symantic Error
Symbolic link
Symphonic Rock
Sync changes
SYNC FREQ
Sync pattern changes
Synchronization type
Syntax Error
Syntax error
Syntax not supported
SynthMaker -
System 5
System b
System data
System Exclusive
System Exclusive End
System gn
System Message
System Reset
System y
System.Generics.Collections iY
System.Generics.Collections y
Tab name
Tablet (alternate)
Tablet (default)
Tablet sensors
TAction EEPLRenderAllTracksTimeSelAction
TAction FileExportProjectDataFilesAction
TAction FileSaveToNewProjectFolderAction
TAction MacroDeclickSelectedSmoothAction
TAction MacroPurgeUnusedAudioClipsAction
TAction OptionsShowAccountSettingsAction
TAction OptionsShowGeneralSettingsAction
TAction OptionsShowProjectSettingsAction
TAction ToolsDumpScoreLog10MinutesAction
TAction ToolsDumpScoreLog20MinutesAction
TAction ToolsDumpScoreLog30MinutesAction
Tail envelope
Tail slope
Target channel
Target control
Target links
Tascam US-428
TBFlatButtonsMenu )
TBoolSetting f
TChildWPForm '
TClipboardFormats 0p
TConversionClassh m
TCustomMEnvelope s
TDataFormatMap Fp
Tempo (BPM)
Tempo (coarse)
Tempo based spline
Tempo information
TEMPO SYNC
Tempo tapper
Tempo value (BPM)
Terms of Use
Test channel 0
Test channel 1
Test message processing
Test passed
Test program expiration
Text align
Text color
Text color (pressed)
Text only
TFLCommandLine P
The venue
Their settings
Theme manager
Theme selector
Theme update
Thick lines
Thread terminated
Throw dice
Thumbnail view
Tiff image
TIFF Images
Timbre decay EQ
Timbre decay time
Timbre EQ shifting
Timbre forced harmonics
Timbre fullness
Timbre impulse EQ
Timbre morphing
Timbre pre-decay
Timbre release EQ
Timbre release time
Timbre rotation
Time based
Time control
Time dist
Time format
Time markers
Time mul
Time seg
Time selection
Time settings
Time signature
Time signature change
Time signature denominator
Time smoothing
Time stretch
Time stretching
Timebase (PPQ)
Timespan too long
Timing Clock
TMenuFinder .
TMP SYNC
TNewsItem U
TO BUFFER
Toggle browser
Toggle browser node
Toggle browser panel
Toggle controls panel
Toggle download
Toggle mute state
Toggle on release
Toggle plugin picker
Toggle project picker
Toggle snap mode
Toggle upload
Too many arguments
Too many requests
Toolbar data
Toolbar presets
Toolbar: Playback mode
Toolbar: Show Mixer
Toolbar: Show Playlist
Toolbar: Song mode
Toolbar: Tempo Selector
Toolbars PPI
Toolbars scaling
Tools 1
Tools 2
ToolsFLCloudDistributionMenu ,
Top 40
Top Left
Top left
Top Right
Top right
Touch controller
Touch Keyboard
Touch keyboard
Touch pads
Touchscreen available,
Touchscreen not available
Toxic Biohazard
Trace: Closed
Trace: Created temporary
Trace: Creating
Trace: Deleting
Trace: Erase failed
Trace: Erasing
Trace: Opening
Trace: Re-entry
Trace: Rename
Trace: Reopening
TRACK AMP
Track latency
Track number
Track options
Track origin
Track routing
Track separators
Traditional time signatures
Transfer cancelled
Transfer extraction failed
Transfer failed
Transfer finished
Transfer finished, extracting
Transient (bleeding)
Transient (no bleeding)
TRANSIENT PROCESSING
Transient Processor
Transistor Bass
Transparent menus
Transparent windows
Trash every
Treble clef
Trebuchet MS
Tree display
TRegGroups 1O
Trial Edition
Trial Limitation
TRIAL VERSION
Trial version
Trigger ahead
Trigger live clips
Trigger note
Trim outside selection
Trim PDC silence
Trim selection
Trim side noise
Trim unused tracks
Triple buffer
Triple saw
Triple saw 2
Triplet sequence
TRttiDataMember UH
TRttiField XH
True PPI
True stereo
Truncate swing notes
Try another edition
Try FL Studio
Ts. Grain
Ts. Smooth
Ts. Time
TUndoItem r
TUnicodeStringsAdapter .q
Turn into marker
Turn into region
Turn off
Turn on
TVSTPlugin - RESTORE
TVSTPlugin - SAVE
TWindowsElectricalSensor 13
Type a progression
Type in (beats)
Type in (BPM)
Type to filter
Typing keyboard
Typing keyboard (disabled)
Typing Piano
Typing to piano
TZipMaster exception
TZMPipeListImp 6
TZXmlWriter: No writehandler
U (slider 1)
U 9JTu:H
U HcR
U(+U A+
UMC UMC UMC
Unavailable device (Port
Undefined error
Underruns 0
Unequal brackets
Unexpected ChanIndex eventA
Unexpected ChanIndex eventB
Unexpected end
Unexpected Fades.Gain
Unexpected Fades.InMs
Unexpected Fades.InTension
Unexpected Fades.OutMs
Unexpected Fades.OutTension
Unexpected Memory Leak
Unexpected RuntimeId
Unexpected variant error
Unicode text
Unicode UTF-7
Unicode UTF-8
Unipolar product
Unison index mapping
Unison off
Unison order
Unison phase
Unison pitch
Unison pitch thickness
Unison scale (unipolar)
UNIT ORDER
Unlinked tracks only
Unlock all zones
Unlock failed
Unlock FL Studio
Unlock in progress
Unlock on web
Unlock sample
Unlock with account
Unlock with file
Unmute all
Unmute all clips
Unmute selected
Unrealistic tempo
Unspecified error
Unsupported clipboard format
Untitled project
Unzipping files
Up keyboard key
Updates and privacy
Upgrade fire edition
Upgrade offers
Upgrade registration
Upgrade reminder
Upload failed.
Upload to
Upload to cloud
Upload to YouTube
Upload URLs retrieved
UploadFile failed
UPPER MID
US: default
Use 32Bit buffer
Use Blue
Use channel declicker
Use CPU
Use Desktop resolution
Use hardware buffer
Use lengths
Use loop points
Use memory (MiB)
Use offline help
Use polling
USE REGIONS
Use riff
Use time offset
Use unique names
User 1
User 2
User data
User data folder
User data location
User defined
User has storage
User name
User not found
User Presets
UserInfo retrieved
UTIcU L
UXD+P D
V (slider 2)
V zoom
V1 Legacy
V2 TB303
Value for
Value for gain
Vanilla all pass
Vanilla band pass
Vanilla band stop
Vanilla high pass
Vanilla high shelf
Vanilla low pass
Vanilla low shelf
Vanilla peaking
Variable time
Variant overflow
VCA SMOOTH
Vectorscope ruler
Vel curve
VEL M
VELO TRACK
Velocity filter cutoff
Velocity layers
Velocity mapping
Velocity splits
Vendor error
Verify plugins
Vertical fader
Vertical meters mode
Vertical scrolling speed
Vertical zoom
VFX Color Mapper
VFX Envelope
VFX Key Mapper
VFX Keyboard Splitter
VFX Level Scaler
VFX Script
VFX Sequencer
Video bitrate
Video codec
Video cue points
Video export settings
Video preloading
Video wizard
View channel rack
View FPS
View layout
View mixer
View pack
View Piano Roll
View Playlist
View project info
View scratching clock
View settings
View SoundCloud preview
View tempo tapper
View touch keyboard
View voice count
View YouTube preview
Viewport height
Viewport width
VINTAGE CHORUS
Vintage Chorus
VINTAGE PHASER
Vintage Phaser
Virtual Tree Data
Virtual Tree Reference
Virtual Treeview
Visible to
Visualizer off
Vocal denoiser
Voice character
Voice Gain
Voice output
Voice Pan
Voice Pitch
Voice random
Voltage range
VOLUME ART
Volume Env
VST (fx)
VST (gen)
VST 1  2
VST 2
VST instrument host
VST instrument sampler
VST Plugins
VST SAMPLER
VST3 (fx)
VST3 (gen)
VST3 effect
VSTi 1  2
VSTi installation
VU style meter
Waiting for download
Waiting for image-line.com
Waiting for transfer
WARP POINT
Wasp DXi
Wasp DXi Editor
Wav audio data
WAV bit depth
WAV rendering
Wave Candy
Wave slice
WAVE SPACING
Wave Traveller
Wave visualization
Waveform editor
WavPack export
Web link
Welcome to
Welcome window
Western Frisian
WET GAIN
WET ONLY
WET VOL
What's new
When armed
When done
Whiney male
White color
White notes only
White Smoke
Whole keyboard
Whole tone
Wide 2
Wide 3
Wide alt
Wide bump
Wide Hz
Widen stereo
Window mode
Windows -
Windows 10
Windows 11
Windows 2000
Windows 64
Windows 7
Windows 8
Windows 8.1
Windows bitmap
Windows metafile
Windows Server 2003
Windows Server 2008
Windows Server 2012
Windows Server 2016
Windows Server 2019
Windows Server 2022
Windows Shell
Windows VISTA
Windows Vista
Windows XP
With audio
With colors
With project
Word rate (WPM)
Wrap remainder
Write empty SFX
Write file
Wrong IDToChanPat channel
WS MIX ENV
X coordinate
X rotation
X(H+X H
X-Generator: POEditor.com
Xing 2.20
XML Prolog
XY - X
XY - Y
Y coordinate
Y rotation
Yamaha DX7
Yamaha DX7 presets
Yellow Khaki
Yes to All
Z (throttle)
Z rotation (rudder)
Zero-X Grid
ZGameEditor Visualizer
ZipMaster Sniffer
Zipping files
ZMDllLoad C
Zone 1
Zone Env 1
Zone Env 2
Zone Lfo 1
Zone Lfo 2
Zone Name
ZZDC Error
```

---

## Meningar (ledtrådar/status) — 992

```
A call to an OS function failed
A controller's script has been updated
A factory preset with this name already exists
A file or folder with this name already exists.
A function cannot be both virtual and override
A little larger than a dwarf
A template with that name already exists
A.FadeIn should be manual
A.FadeOut should be auto
A.FadeOut should be manual
A.Fades.OutMs not equal B.Fades.InMs
A.InMs should be 0
A.InMs should be kept
A.OutMs should be 0
A.OutMs should be set to 0
Abort this file transfer
Absolute speed based spline
Act as regular ReWire device
Adapt to current tempo
Adding target links to automation
Adds all core program features.
Adds all native plugins.
Adds more of our most popular native plugins.
Affects only frequencies around the cutoff
After reset A.OutTension should be 0
After reset B.InTension should be 0
Akai FL Studio Fire
All playlist track groups
Allow FL Studio to undo when Ctrl+Z is pressed
Allow resizing from left
Also include audio when preloading video
Alt solo selected track(s)
Always save to the global folder
Always send notes to bus
Always show advanced controls
Always show external input levels
Always show MIDI import window
An error occurred while reading the FLP file.
An optional feature was updated.
An unexpected error occurred.
An unexpected memory leak has occurred.
Animation timer set to
Another instance is already open.
Argument out of range
As a registered customer we can serve you better
ASIO driver is not open.
Ask for data folder
Associate project files with application
Audio clips are not available
Audio logger is empty
Audio recording feature not available
Audio, into the playlist as an audio clip
Auto accept detected controller
Auto keep long audio on disk
Auto learn next chord
Auto map (white + black)
Auto map (white only)
Auto name effect slots
Auto name playlist tracks
Auto select linked modules
Auto switch display filter
Auto switch to external input only
Auto zip empty channels
Auto zoom in piano roll
Auto-slice with dull sensitivity
Auto-slice with medium sensitivity
Auto-slice with sharp sensitivity
Automatic chord (bottom up)
Automatic chord (top down)
Automatically check for updates
Automation clips merging approximation
Available to buy for owners of FL Studio
B.FadeIn should be auto
B.FadeIn should be manual
B.FadeOut should be manual
B.InMs should be 0
B.InMs should set to 0
B.OutMs should be 0
B.OutMs should be kept
Back up every save
Back up new projects to FL Cloud
Back up to FL Cloud storage
Band solo on Shift
Bar : Beat : Tick
Based on current Windows font size
Based on the free open source ZGameEditor engine
Bass: one octave down
Bass: one octave up
Be sure you're using at least DirectX 5.
Beats per bar (time signature numerator)
BeatSlicer gap filling, alternate
Benchmark note list calculation
Bitmap image is not valid
Blend with existing data
Break can only be used in loops
Bridge process destruction timed out
Browser enable auto indexing
Browser extra search folders
Browser FL CLOUD displayed
Browser full sample preview
Browser plugin factory tags
Browser search aborted by user.
Browser tab icon style
Burn all looping channels
Burn selected loops to pattern
Buy FL Studio and upgrades
By mixer track number
C-1 .. G9 (mid C4)
C-2 .. G8 (mid C3)
C.InMs should be 0
C.OutMs should be 0
C0 .. G10 (mid C5)
Center dead zone (large)
Center dead zone (small)
Channel envelope preset change
Channel sample was changed
Channels in selected bus
Channels routed to this track
Check for launchpad hidden item
Check if all plugins exist
Check properties on display change
Chord Progression Tool opened
Clean up plugin names
Click and drag to move layer
Click Generate to make a new one.
Click the Loop Starter icon to open Loop Starter
Click the Play button to hear the loop.
Click the top dice to change all loops.
Click to expand layer
Click to load a sample
CLICK TO LOAD SAMPLE
Click to open the download page in a web browser
Click to show more information in a web browser
Click Update to renew your license.
Click-and-hold  special gesture functions
CloseWAVFile: WAVSTREAM IS NIL
CME UF Omnipotent Master Keyboard
Collapse all grouped tracks
Collect project data files
Color per voice (trigger only)
Combine 32 and 64 bit versions of a plugin
Combine VST and VST3 versions of a plugin
Compensate for master latency
Connect to FL Cloud
Connection has timed out
Connection reset by peer.
Continue can only be used in loops
Continue in trial mode
Continue the unlock process there.
Couldn't access preset file.
Couldn't access the preset file.
Couldn't convert to automation clip
Couldn't create the DirectSound main object.
Couldn't create the FL Studio Remote server
Couldn't create the primary DirectSound buffer.
Couldn't create the streaming ASIO buffer.
Couldn't init the ASIO driver.
Couldn't link to the plugin.
Couldn't open ILVideo library
Couldn't open the ASIO driver.
Couldn't open the plugin for an unknown reason.
Couldn't retrieve chain information
Couldn't set the DirectSound cooperative level.
Crash in param change
CreateStorage failed: zone is empty
Cuts frequencies around the cutoff
Data not multiple of Block Size
Database must be specified.
Debug version, NOT FINAL
Deck A - reversed
Deck B - reversed
Declick in all regions
Declick out all regions
Default fast 12dB low pass
Default large male voice
Default to use MIDI Out
Delay playback until that time
Detach all new plugins
Details about current project
Detection for songs with constant tempo
Detection for songs with variable tempo
Device used by another application
DigiCert Assured ID Root CA0
DigiCert Trusted Root G40
Disconnect (client) calls ExecuteOnGUIThread
Display normals on meshes
Display preset name in preset selector
Display selection rectangle in channel rack
Display selection rectangle in mixer
Display selection rectangle in mixer dock
DJ TechTools Midi Fighter 3D
Do not show this in the future
Don't ask this in the future
Don't copy the data
Don't initialize controls automatically
Don't limit windows to screen
Don't move or copy
Don't open the project
Don't render the tail (rendering loops)
Don't save the project
Don't save to the folder
Don't set the tempo
Don't show MIDI import window
Don't show this in the future
Don't show this warning anymore
Don't use a data folder
Double-click on audio clips
Double-clicking handling for MIDI
Download a new license
Download copy and open
Download of a Loop Starter sample failed
Downloading and installing your license
Downloading and installing your license.
Dropping on track header locks to content
Drum and bass (half-beat)
Drum and bass (var)
DX7 preset VERY APPROXIMATE conversion
DXi Test - Editor 1
Easier debugging for scripting
Echo - ping pong
Elastique efficient, tonal mode
Elastique efficient, transient mode
Elastique Pro with formant shifting
Elastique pro, default mode
Elastique pro, transient mode
Elastique soloist, monophonic mode
Elastique soloist, speech mode
Elastique version 2, default mode
Elastique version 2, monophonic mode
Elastique version 2, speech mode
Elastique version 2, transient mode
Elastique version 3, default mode
Elastique version 3, monophonic mode
End of startup log
Ensure plugins still exist
Ensure processor state in callbacks
Ensure small gap between notes
Ensure Valid Note Record
Enter a template name
Enter rhythm preset name
Enter the number of mixer tracks
Enter the number of mixer tracks to add
Enter the verification code here
Enter URL to stream
Enter your 20 digit code
Enttec DMX USB Pro Mk2
Env filter - Default.fnv
Events - level scaling
Exception in safecall method
Expand all grouped tracks
Expect new events to have no fades
Expected equal tensions with Shift
Expected Helper.CurrentHandle to be fhGain
Expected Helper.CurrentHandle to be fhLeftTime
Expected Helper.CurrentHandle to be fhRightTime
Expected mirrored tensions without Shift
External and mixer input
External and mixer input only
Extract root key from filename
Extract stems from sample
Fade in curve shape
Fade out curve shape
Fast declick for cut groups
Fast forward song pos
Fat - 8 voices
Fat - detuned + time offset
Fat - more detuned
Field cannot have these modifiers
File does not exist
File is not a RIFF file
File may be corrupt or saved in wrong format.
File must be open
File must be open for writing
File name and location
File name is empty
File name too long.
Filename index out of range
Filtering mode to use for imported images
Filters low frequencies starting at the cutoff
Finished (failed to find import function)
Finished (Scan - internal)
Finished (store all presets)
Finished (TVSTPlugin - SAVE)
Finished listing MIDI devices
Finished updating sample pool
Fire: Change step shift
Fire: Step seq edit
First Fade-in should remain auto
Fit to current tempo
FixedRows can only be 0 or 1
FL Cloud Download Placeholder
FL Cloud mastering v2 Windows
FL Cloud subscription required
FL Cloud upgrade needed
FL Studio All Plugins Edition
FL Studio channel state file
FL Studio Cloud Storage error
FL Studio DXi Editor
FL Studio Fruity Edition
FL Studio is in Trial Mode.
FL Studio license plate
FL Studio loop starter scripts
FL Studio MIDI Controller Scripts
FL Studio Midi Sync warning
FL Studio Mobile Factory Data
FL Studio Mobile Rack Data
FL Studio Optional Features
FL Studio Plugin Scanner
FL Studio Producer Edition
FL Studio project file
FL Studio Remote Scripts
FL Studio Remote server created
FL Studio Remote server destroyed
FL Studio Remote: found
FL Studio Remote: Importing modules
FL Studio score file
FL Studio Signature Bundle
FL Studio trial selection
FL Studio user name
FL Studio VSTi (
FL Studio Web is our online DAW.
FL Studio Web projects
FL Studio Web Storage error
FLAC only allowed for .mp4 video file extensions
Floating point division by zero
Focus selected channel on playback
Foot pedal controls note off
Force high performance power plan
FPS is not set to a valid value
Frames per second (right-click for custom)
Free to download for owners of FL Studio
Fruity 7 Band EQ
Fruity 7 Band Equalizer
Fruity Parametric EQ 2
FullRecursive not allowed with empty name
Function should not return a value
Gain should be 1.0
Gain should pause at 0dB (1.0)
Generate progression from text
Generate steps for all channels
Generic jog (can be used to select stuff)
Global envelope attack scale
Global envelope attack time
Global envelope decay scale
Global envelope release scale
Global envelope sustain offset
Go to Technical support
Graph editor - remove step
Graph editor add step
Guess the length of this clip
Hardware refresh mixer track
Hercules DJ Console MK2
Hercules DJ Control MP3
High pitched, quavering voice
High pitched, sung voice
High quality realtime stretching
Highlight a musical scale
History of used files
Hold Shift and click to select layered channels
Hold to keep at velocity
HOW TO USE STEP SEQUENCER
HQ for all plugins
Huge waste of CPU
I agree to Terms of Use
I agree to the Image-Line
I Fixed it, Try Again
I have a license
Icon image is not valid
Ignore space bar key
Image freq local mask
Image freq pixel scale
Image-Line RSS News Feed
Image-Line waiting cursor animation
Increase live loop mode
Increase live pos snap
Increase live trig mode
Increase live trig snap
Index out of range
Initial gain should be 1.0
Initialize the help file
Initialize the plugin scanner
Installation of the license failed.
InstanceInfo not assigned - EXIT
InstanceInfo.Factory not assigned - EXIT
Instrument tracks are not for audio clips.
Internet connection has been lost
Interpolate (white + black)
Interpolate 1 to 8
Interpolate between banks 1 and 8
Into playlist as an audio clip
Is device midi out assigned
Is grid bit assigned
Is higlight rectangle defined
Is in popup menu
Is loop recording enabled
Is start on input enabled
Is time time display set to minutes
It can be downloaded from our website.
Its settings are not loaded.
ITU-R BS.1770 based loudness measurement
Jump to arrangement marker
Keep links of the first selected clip
Keep the groove only
Key to start (sample time, C5 to C7)
Key to start (song time)
Key to start (transient time)
Label with missing definition
LAME in FL Studio 2026
Last 2 tweaked parameters
Last Fade-out should remain auto
Last FL Cloud Data Retrieval
Last FL Cloud date
Launchpad XY - X
Launchpad XY - Y
Layer selections in both decks
Learn about the editions
Learn from MIDI input
Leave ending time unchanged
Leave the rendered tail (rendering songs)
Leave unedited audio data on disk
Left offset, main out
Left offset, out 2
Left offset, out 3
Left offset, out 4
Left scale, main out
Left scale, out 2
Left scale, out 3
Left scale, out 4
Less than a minute was spent working on it.
License should be renewed
Like a treble EQ
Limit to 1 voice per key
Linear phase mod precision
Linear phase mod speed
Linear-phase filters (legacy mode)
Linked device not assigned
List piano roll chords
Live block number to time
Live time to block number
LoadHostDLL failed - EXIT
Loading aborted. Project cleared.
Loading project, hold ESC to abort
Loading your FL Studio Web projects
Loading your Project backups
Lock current chord (prevent from regenerating)
Lock to safe colours
Log out all users
Log out this user
Look for new projects
Lowest dB range of the spectrum analysis
Major scale random notes per syllable
Make a lightweight clone of a preloaded video
Make clip unique when not used
Make selected clips unique
Make sure you can see the Playlist.
Make unique as sample
Manage FL Cloud plugins
Manage your Rent-To-Own license
Manual fade in flag after edit
Manual fade in flag after edit + return
Manual fade out flag after edit
Manual fade out flag after edit + return
Manual fade out flag before edit
Mapping of remote controllers
Mastering is an FL Cloud powered tool.
Mastering is not initialized
Maximize to all monitors
Maximum amount of backups that are auto-saved
Maybe the samplerate is unsupported.
Mellower with breath noise
Menu index out of range
MergeClips resulting channel is not assigned
MergeClips resulting sample wasn't be loaded
Metafile is not valid
Meter gradient color +0dB
Meter gradient color +6dB
Meter gradient color -0dB
Meter gradient color -6dB
Meter gradient color -INFdB
Method is not marked as virtual in base class
Microsoft Windows .NET Server 2003 family
Microsoft Windows 95 OSR2
Microsoft Windows Millennium Edition
MIDI CC centered around 63
MIDI CC centered around 64
MIDI controller support forum
MIDI Out with Fruity LSD
MIDI score export mode
Mini piano roll preview
Minute : Second : Centisecond
Minute : Second : Millisecond
Mix audio. Only one in each project.
Mix in buffer switch
Mixed (folders and files) view
Mod X speed mapping
Mono - Left only
Mono - Right only
Morph duration with closest note's duration
Morph oscillators 1 to 5
MS Shell Dlg 2
MTC 24 fps (
MTC 24 fps (film)
MTC 25 fps (EBU)
MTC 29.97 fps (
MTC 29.97 fps (drop)
MTC 29.97 fps (drop-frame) (NTSC)
MTC 29.97 fps (non-drop frame) (NTSC)
MTC 29.97 fps (non-drop)
MTC 30 fps (
MTC 30 fps (drop)
MTC 30 fps (drop-frame) (NTSC)
MTC 30 fps (non-drop frame) (NTSC)
MTC 30 fps (non-drop)
Multithread plugins on this map
Name already defined in this scope
Name of new preset
Native Instruments MASCHINE JAM
Native Instruments MASCHINE MIKRO
Native Instruments TRAKTOR KONTROL F1
Natural EQ mapping scale
Natural EQ mapping weighting
Never (just remind every 10 minutes)
Next to last tweaked
Noise profile not acquired
Not a floating point value
Not all bytes sent.
Not signed in to FL Cloud
Note filter cutoff frequency
Note filter resonance (Q)
Note length in note properties
Note name preset files
Note on velocity mapping
Notify about parameter changes
Notify about rendering mode
Novation Launch Control XL
Novation Launchkey MK2 (keyboard)
Novation Launchkey MK2 (performance)
Nudge clips with mouse wheel
Nudge notes with mouse wheel
Number of bands used for the frequency analysis
Number of events in NoteList at time 0
Number of events in OmniList at time 0
Number of tracks to add
Observer is not available
Occasionally (every 10 minutes)
Off (rely on ramping)
Offset all levels by value
OmniEvents ChanID in range
Only available in Producer Edition or above.
Only available in the Fruity Edition or above.
Only available in the Producer Edition or above.
Only expand one layer at a time
Only function definitions are allowed here
Only one default statement is allowed.
Opening MIDI input devices
Opening MIDI output devices
Operation already in progress.
Operation now in progress.
Operation unsafe at current time
Original FL Studio system
Out only (no bleeding)
Package Size Too Big.
Patcher received invalid parameter value from FL
Patcherizing will lose automation links
Path ended with an open bracket
Path ended with an open string
Pattern or song mode
Persistent arrays of this datatype not supported
PHASE + PHASE ENV
Piano roll - arpeggiator
Piano roll - articulator
Piano roll - chopper
Piano roll - claw machine
Piano roll - flamer
Piano roll - level scaling
Piano roll - LFO
Piano roll - quantizer
Piano roll - randomizer
Piano roll - riff machine
Piano roll - score flipper
Piano roll - strumizer
Piano roll or event editor
Piano Roll tools menu
Pinvalid distance too far back
PITCH + PITCH ENV
Pitch analysis after sample load
Pitch selected clips one semitone down
Pitch selected clips one semitone up
Pitch up odd beats
Pivot the spectrum at 1kHz
Plane(s) to work on
Playback position is coarse mixer position
Playback position is estimated
Playback position is retrieved from driver
Please adjust your audio settings.
Please choose another project name.
Please download it again from your user account.
Please enter a valid code.
Please make sure it exists and try again.
Please purchase Lifetime free updates.
Please register for better customer support
Please remember to register
Please report it to Technical Support.
Please report this on our Techsupport forum.
Please restart FL Studio and try again.
Please select at least two automation clips.
Please select different references for A and B.
Please stop playback and try again.
Please stop recording before extracting stems.
Please stop recording to export playlist tracks.
Please stop recording to master the project.
Please take the time to register your boxed copy
Please try again later
Please try another sample rate.
Please update to newest version.
Please update your license to master this track.
Please use about screen to register.
Please use other plugins instead.
Plugin crashed when it was opened
Plugin is not stereo
Post effects, pre fader
Post level and panning
PPMd version I, Rev 1
Prepare for MIDI export
Prepare for smooth editing
Prepare time envelope for drumloop
Preserve controller default mode
Preset name - AuthorInfo line 1Info line 2
Press again to switch
Press Play to listen to the progression.
Preview browser menu item
Preview from this chord
Preview up to current step
Printer index out of range
Printer selected is not valid
Problem in directory watcher.
Problem saving the project
Problems loading the project
Process inactive inputs and outputs
Process maximum size buffers
Project data size warning
Prompt array must not be empty
Property cannot be translated.
Protocol wrong type for socket.
Provided host name was not able to be resolved
Pulse length in samples
Purge unused audio clips
Put markers at large grid lines
Put markers at medium grid lines
Put markers at small grid lines
Put undone recordings in the recycle bin
Quick estimation for short loops
Quick quantize start times
Quick way to add a plugin to the database
QuickRelease on a clip with fade-out
Rarely (every 15 minutes)
Read beyond end of file
Read sample root note
Read sample tempo information
Reader has no more rows
Recording feature not available
Recording has been cancelled.
Register your box and get Lifetime Free Updates
Registering your box is securing your copy
Regularly (every 5 minutes)
Reload all images (clear memory cache)
Remap zone(s) as drumkit
Remember size for this tab
Remote find event value
Rendering single fades 1
Rendering single fades 2
Rendering single fades 3
Rendering single fades 4
Rendering single fades 5
Repeat slices four times
Replacing samples in all Loop Starter channels
RES + RES ENV
Rescan plugins with errors
Rescan previously verified plugins
Resolution (right-click for custom)
Resolution is not set to a valid value
Restart on new block
Restart on new chord
Restarting FL Studio may resolve the issue.
Return true when menu is showing
Returns if the program is playing or stopped
Returns true if graph editor is visible
Rework the existing progression
Rich Text Format Without Objects
RichEdit line insertion error
Right offset, main out
Right offset, out 2
Right offset, out 3
Right offset, out 4
Right scale, main out
Right scale, out 2
Right scale, out 3
Right scale, out 4
RMS based A-weighted loudness measurement
Run last script again
Running tests, hold ESC to abort
Safe to edit stuff
Sample rates above 48000Hz use more CPU.
Sample start covers whole length
Samples used in project
Saving project, hold ESC to abort
Score log clearing confirmation
Script executed without errors
Scrolls to reach time markers
Seamless cuts where possible
Searching, hold ESC to abort
Separate slices in the stereo field
Shift slices once down
Shift slices once up
Side by side full
Side by side half
Sidechain selected to this track
Sidechain selected to this track only
Sidechain this track to selected
Sidechain this track to selected only
Sidechain to this track
Sidechain to this track only
Sign in and unlock
Sign in to import your FL Studio Web projects
Sign in to load your Project backups
Sign into your account to activate FL Studio.
Sign up or unlock
Signed in to FL Cloud
Similar to a bass EQ
Sine (for FM operators)
Slave playback to host
Slave to host playback
Slow down within loop region
Small scrollbars in editors
Small text in plugin list
Smooth up abrupt changes
SMPTE (23.976 fps DROP)
SMPTE (24 fps NONDROP)
SMPTE (25 fps NONDROP)
SMPTE (29.97 fps DROP)
SMPTE (30 fps NONDROP)
SMPTE 29.97 fps (drop)
SMPTE 59.94 fps (drop)
Socket operation on non-socket.
Solfege (do, re, mi, ...)
Some Plugin or Other
Something went wrong while mastering.
Song has no marker
Specified error number is out of range
Spectral mask - default.fnv
Spectrum gradient color 1
Spectrum gradient color 2
Spectrum gradient color 3
Spectrum gradient color 4
Spectrum gradient color 5
Springback: fade-in should be pushing
Springback: fade-in should be squeezed
Springback: fade-in should spring back
Springback: fade-out should be pushing
Springback: fade-out should be squeezed
Springback: fade-out should spring back
SSL certificate has been revoked.
SSL certificate has expired
SSL certificate is not valid
SSL client certificate contains errors
Standard MIDI clock and Song Position Pointer
Start a new project
Start listing MIDI devices
Start recording when input is detected
Start the MIDI sync thread
Starts at octave 2
Starts at octave 3
Starts at octave 4
Starts at octave 5
Starts at octave 6
Step Seq Advanced Fill
Step sequencer mode zoom
Steppy (cents to semitones)
StepSeq 808 kick Fill4
StepSeq 808 snare Fill8
Store in spare state
Sub-menu is not in menu
Summary copied to clipboard
Switched to song mode
Sync video with song position
Syntax error in expression
Tag to menuOp correspondence
Tag to menuOp mismatch
Tags and Favourites cannot be changed.
Tension A-out should be same as B-in (X)
Tension A-out should be same as B-in (Y)
Test log copied to clipboard
That means you haven't input any notes yet.
The AnsiString was not zero terminated
The application will now close.
The component state wasn't restored.
The controller state wasn't restored.
The downloaded license is incorrect.
The email is not verified.
The input file doesn't exist
The license was copied to all users.
The license was installed.
The license was not copied.
The location sensor is started
The Master mixer track is muted
The mastered file is being saved
The mastered file was saved to
The mastered files are being saved
The mastered files were saved to
The MIDI input device couldn't be found.
The MIDI input device isn't valid.
The MIDI output device couldn't be found.
The MIDI output device isn't valid.
The new project folder could not be created.
The new project folder is not empty
The new template could not be created.
The output files already exist.
The preset name is already used.
The program does not expire on the correct date.
The program will expire while it should not.
The program will not expire while it should.
The project folder does not exist
The resulting MIDI file will probably be empty.
The riff type id was deemed invalid
The selection contains overlapping clips.
The selection may only contain automation clips.
The settings have not been loaded.
The settings may not have been saved.
The state wasn't saved.
The terms of use have changed.
The transfer was aborted
The translation for the language code
The validation file has been saved.
The voice of the Colossus computer
There are no events to convert.
There are unsaved changes.
There is no help available for this effect
There is nothing to rename.
These settings requires restart to take effect
These settings were saved with a newer version.
Thin line representation of frequencies
This can not be undone.
This feature is not available in DW Player.
This folder can't be used for user data
This is a factory script, it can't be edited.
This is the default text.
This may take a while
This plugin does not support surface instances.
This plugin has expired.
This plugin's editor window is detached.
This program must be run under Win64
This project started on
This sample is too short for tempo detection.
This test needs 4 channels
This will spare CPU
Threaded project sample loading
Timbre 1  2 mix
Timbre 1 harmonic level
Timbre 2 harmonic level
To be reset when the song starts
Toggle browser and options panel
Toggle channel mute state
Toggle channel solo state
Toggle track armed state
Toggle track enabled state
Toggle track mute lock state
Toggle track mute state
Toggle track reverssed polarity state
Toggle track slots enabled state
Toggle track solo state
Toggle track swap channels state
Too many array dimensions
Too many open files
Too many open files.
Too small for span sfx
Toolbar: Show Channel Rack
Toolbar: Show Piano Roll
Touch-sensitive jog in jog mode
Trace: Creating temporary name failed
Trace: Set file Date
Track inspector on left side
Tracks routed to this one
Tracks skipped while rendering
Tracks this one is routed to
Transport keystrokes to host
Trim all noise  slice up
Trim unused mixer tracks
Try the 32bit version of the plugin instead.
Try to load it as a 32bit plugin first
Try to load it as a 64bit plugin first
Try to prevent popup windows
Turn all points smooth
Turn EQ into impulse response
Turn into audio clip channel
Typing keyboard to piano
Typing keyboard to piano keyboard
Typing to piano keyboard
Undoing an entry with a take that was discarded
Unexpected manual fade in time
Unexpected RecChan.Count after InsertTimedEvent
Unlinked and non-empty tracks
Upgrade Fire to Producer Edition
Upgrade Fire to Signature Bundle
Upgrade Fruity Fire to All Plugins Edition
Upgrade Fruity to All Plugins Edition
Upgrade Fruity to Producer Edition
Upgrade Fruity to Signature Bundle
Upgrade Producer to All Plugins Edition
Upgrade Producer to Signature Bundle
Upgrade Signature to All Plugins Edition
Upgrade to latest version
Upgrade to unlock all languages
Upload to 3rd party service
Use a dedicated ASIO driver for best results.
Use a unique name
Use both mouse buttons in step sequencer
Use current layout when changing presets
Use current time signature
Use data outside selection
Use default param type
Use external carrier source instead
Use final (post master) audio
Use FIR downsampling filter
Use fixed size buffers
Use FL Studio VSTi instead.
Use main snap setting
Use maximum buffer size from host
Use OpenGL 4x multisample for smoother graphics
Use polyphase downsampling filter
Use project tempo on load
Use project tempo when loading a sample
Use sample built-in slicing information
Use Shift + mousewheel to nudge clips
Use Shift + mousewheel to nudge notes
Use tablet PC or digitizer stylus
Use the project name
Use the same bit size as the host
Use this to spare CPU
Used to relocate items
User data folder located in cloud storage
User data folder warning
V2 Extended cutoff range
V2 TB303 cutoff range
Value for automation point
VendorLib must be specified
View presets in folders (if any)
View with folders left, files right
View with folders left, grid right
View with folders left, samples (player) right
Visit our online shop
Wait for input to start playing
Warn when closing Chord Progression Tool
Warning: Set file Date
Warning: the TS404 module is now deprecated.
We will remind you again in one month.
We will remind you again in two months.
We will remind you again next week.
We will remind you again tomorrow.
Welcome to FL Studio
Welcome to the Chord Tool
Western DOS Latin 1
When recording or not playing
When starting a new project only
Wide Hz scale (unipolar)
Windows Registry Editor Version 5.00
Windows Server 2003 R2
Windows Server 2008 R2
Windows Server 2012 R2
Work on existing score
Wrap the rendered tail (rendering loops)
Write detached SFX stub
Write expression for live audio output
Wrong datatype for binaryop
Wrong nr of arguments
You are advised to restart FL Studio.
You can only have one surface in the form.
You don't have any projects yet.
You have not verified your e-mail address yet.
You have the maximum number of layers
You must enter your e-mail address
You must enter your password
You must have a least one chord selected.
You must stop recording to rename the project.
You shouldn't be seeing this
You will be notified when it is ready to use.
You will need this file to download a license.
Your license has expired
Your license has expired and
Your license should be renewed
Your license was renewed
Your web browser has been opened.
Zero-cross check all regions
```

---

## Interna handlernamn (avslöjar funktionsområden) — 5

```
AllAudioClipsStretchMenu
Control Change
Fire: Change
Program Change
Song Select
```

---

## Fel- och kontrollsträngar — 365

```
Access denied.
Access violation
Affected patterns:
Also delete associated:
Also delete or reset the associated:
Array access outside range:
Array size must be a constant expression
Audio input source:
Audio input:
Audio output:
AVX    support:
Buffer length:
Can only append to one-dimensional array:
Can't animate parameter now
Can't decimate
Can't delete point
Can't drop on an instrument track
Can't drop on instrument tracks
Can't load more than 20 effect plugins at once.
Can't load more than 20 instruments at once.
Can't merge
Can't perform lossless merge
Can't write to file
Can't write to files
Cannot access private field:
Cannot add more automatic chords.
Cannot assign readonly property identifier:
Cannot call private method:
Cannot convert
Cannot convert between different classes:
Cannot create memory map on file
Cannot drag a form
Cannot find file:
Cannot get address of expression:
Cannot map file
Cannot open file
Cannot override without baseclass
Cannot read -Infinity value
Cannot read Infinity value
Cannot read NaN value.
Cannot undo while recording.
Cannot use this operator on a float-expression
Cannot use this operator on a mat4-expression
Cannot use this operator on a string-expression
CLAP plugin failed init function:
CLAP plugin failed loading:
Class must inherit TCommand:
Class not found:
Clip trigger:
Cloud download error:
Command to execute:
Constant expression expected:
Constant expression is not of the expected type:
Constructor must have same name as class:
Could not connect to the server
Could not determine type:
Could not find browser tree node with name
Could not initialize Python.
Could not load BeatTracker module.
Could not load stem extractor module.
Could not open ASIO control panel:
Could not open MIDI device
Could not open video.
Could not open:
Default pitch :
Definitions file not found:
Device hardware id query received:
Device hardware id query sent to device:
Division by zero
Download failed. Error:
Duplicate FileName:
Error compiling expression
Error during execution - processing now disabled
Error during file upload:
Error during token exchange:
Error in
Error in MixThread
Error loading CLAP plugin:
Error loading mastered sample
Error loading plugin's test project
Error loading sample...
Error message is:
Error parsing null value.
Error reading zip file
Error when deleting preset.
Error when linking shader program
Error when processing CPT
Error while accessing the ASIO driver.
Error while creating the notification events
Error while loading the file.
Error while saving the file.
ERROR: load Loop Starter sample:
Estimated maximum size:
Estimated size:
EXCEPTION (VerifyPlugin):
EXCEPTION in TVSTPlugin.SetActive:
Exception when saving the plugin list:
Extended Instrument:
Fade in:
Fade out:
Failed accessing device
Failed autosaving to
Failed encoding.
Failed exporting.
Failed opening AudioRestore plugin - not valid.
Failed opening AudioRestore plugin.
Failed saving.
Failed to add a channel
Failed to clone audio track
Failed to clone instrument track
Failed to clone track
Failed to close
Failed to copy license
Failed to deref
Failed to download the license.
Failed to init deverb module
Failed to initialize mastering
Failed to insert track
Failed to load an old Loop Starter sample
Failed to load QuickFontCache
Failed to Load Stream
Failed to log in to the mastering service
Failed to open
Failed to open an empty project
Failed to open the denoise engine.
Failed to save downloaded file:
Failed to save:
Failed to send analytic data to the server
Failed to unlock the program.
Failed to unzip downloaded file:
Favorites :
File not found:
FL Sound: SampleData, error:
FL Sound: SampleData, wrong type:
FL Studio Midi scripting version:
FL Studio Remote scripting says:
FL Studio Remote:
FLP Saving failed with error code:
Folder added to tab:
Folder already exists in tab:
Folder not in cache:
Function in expression must return a value:
GetStringProperty called with invalid propname:
Grid offset:
Identifier is not an array:
Invalid access
Invalid address expression:
Invalid argument
Invalid argument to date encode
Invalid argument to time encode
Invalid argument.
Invalid binary op
Invalid characters in file name
Invalid characters in path
Invalid code.
Invalid conversion:
Invalid data
Invalid data was read)
Invalid datatype for jump
Invalid datatype for select:
Invalid date
Invalid device ID
Invalid DIB
Invalid Edition
Invalid event ID
Invalid filename
Invalid filespec:
Invalid func op
Invalid identifier:
Invalid image
Invalid image size
Invalid ImageList
Invalid InApp regcode download:
Invalid isolation level
Invalid json format:
Invalid jump op
Invalid length
Invalid license download:
Invalid literal:
Invalid marker index
Invalid MIDI output for
Invalid mixer track nr
Invalid modifiers for function
Invalid note index
Invalid nr of arguments:
Invalid numeric input
Invalid ordinal
Invalid parameter
Invalid pixel format
Invalid pointer operation
Invalid precision
Invalid property path
Invalid property value
Invalid reference
Invalid sample
Invalid slot number:
Invalid source array
Invalid stream format
Invalid string comparison
Invalid StringBaseIndex
Invalid syntax
Invalid time
Invalid track number:
Invalid transaction id
Invalid transaction Object
Invalid type
Invalid type:
Invalid username or password
Invalid value
Invalid variant operation
Invalid variant type
Invalid variant type conversion
Invalidate editor
IPP function not found:
Library data loading failed. Error:
Link key to:
Loading FLP file:
Main GUI scaling factor:
MANY THANKS TO :
Mem usage:
Method not found:
Missing argument
Missing data
Missing external input
Missing FLEX pack
Missing op in binary expression
Missing operand between )(
Missing parameter marker
Missing plugins help
Missing project url
Missing sample
Missing samples info
Mixer :
Mixer threading map rebuilt:
Mixer track map:
Name already defined:
Name not found:
Next progression:
No Backups
No buffer space available.
No channels
No chord (flat)
No data
No data will be sent
No decay (sustain)
No demo projects were found
No effect
No error
No file is loaded
No further information is available.
No ghost notes to show for the current pattern
No Grid
No hold
No input port set (click to auto assign)
No input selected
No limit
No method with this name in base class
No more rows
No NDI sources found
No news at this time.
No parameter
No parameter selected
No plugins were found
No punches
No repeat
No route to host.
No saturation
No SensorManager implementation found
No shift
No snap
No statement to execute
No steps set
No table
No tempo
No thanks
No to All
No token to close.
No video
No voicing, all aspiration
NR OF SAMPLES:
Opening ASIO driver:
OS Version:
Out of memory
Out of system resources
Pattern group:
Please pick a folder that is not a subfolder of:
Plugin locator says:
Poly tilting mode:
POS SYNC:
Previous progression:
Range check error
Range error
Registered to:
REVERB BY :
Root note:
Sample not found:
Search time:
Selected tracks:
Server error:
SetNumericProperty called with invalid propname:
SetObjectProperty called with invalid propname:
SetStringProperty called with invalid propname:
SetStringProperty tried to set duplicate name:
Shader fail:
Skipped files:
SSE    support:
SSE2   support:
SSE3   support:
SSSE3  support:
Storage space used:
Support and help here:
Tab state saved:
Tablet mode changed - slate:
Task result:
Temporary folder for this session:
The connection to this plugin was lost:
The unexpected small block leaks are:
There was an error during scan:
These plugins are:
This file is not a valid GNU gettext mo file:
This plugin failed to load:
This plugin is:
This plugin wasn't found:
Thread crashed:
Threaded generator(s):
Time out:
Total time:
Trace: FileCreate Failed:
Trace: FileOpen Failed:
Traceback (most recent call last):
TRIG SYNC:
Unable to continue loading.
Unable to create directory
Unable to create folder for auto save:
Unable to create folder for recorded wave file:
Unable to create folder to open mix state:
Unable to create folder to save a mixer state:
Unable to insert a line
Unable to open Index
Unable to parse text:
Unable to Replace Image
Unable to save downloaded update.
Unable to unzip update.
Unknown error
Unknown event
Unknown exception
Unknown failure during mastering
Unknown failure when initializing mastering
Unknown file size
Unknown format
Unknown function:
Unknown identifier:
Unknown property:
Unknown type
Unknown type:
Unknown version of Windows
Unsupported operator for value expression:
Unsupported operator:
VELOCITY LAYERS:
Video export failed:
Warning - Wrong mixer track index:
Wrong nr of array indices:
Wrong nr of items in initializer list:
Wrong type of constant expression:
You need Windows 64bit to load this plugin:
```

