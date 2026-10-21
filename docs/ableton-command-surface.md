# Ableton Live 12.4.5 — hela kommandoytan

Ur `Ableton Live 12 Suite.exe` (141 MiB, 454 338 ASCII- och 10 748 UTF-16-strängar).
Ableton namnger sina gränssnittssträngar med prefix, och det gör inventeringen exakt i stället
för gissad: **`sMenu*`** är menykommandon, **`sDialog*`** dialoger, **`sStatus*`** statusraden och
**`sInfoText*`** är ledtrådarna (tooltips). Namnen nedan är prefixet borttaget och CamelCase
uppdelat — inget är påhittat och inget är bortvalt.

| Prefix | Poster | Vad det är |
| :--- | ---: | :--- |
| `sMenu*` | 725 | menykommandon (Create, Edit, View, Options …) |
| `sDialog*` | 523 | dialoger och deras texter |
| `sStatus*` | 200 | statusradsmeddelanden |
| `sInfoText*` | 2302 | ledtrådar/tooltips |

Värt att veta: Ableton skeppar också **sina egna UI-testers reguljära uttryck** i binären
(`^I Capture and Insert Scene$`, `^the Groove Pool should( not)? be open$`) — de ligger inte i
någon av listorna nedan eftersom de inte är kommandon, men de avslöjar att gränssnittet testas
med regexar mot åtkomliga element.

## Menykommandon — 725

Alla `sMenu*`-symboler, en per rad.

```
Ableton Link Audio
Ableton Link Start Stop Sync
Accessibility
Accessibility Speak Extended Note Description
Accessibility Speak Menu Commands
Accessibility Speak Parameter States
Accessibility Speak Slider Ranges
Accessibility Speak Time In Seconds
Activate Loop
Adaptive Q Affects Shelves
Add Folder To Browser
Add Lanes For Automated Parameters
Apply Algorithm
Arm Exclusive
Arm Record In Arrangement
Arrangement In Out
Arrangement Overdub
Arrangement Returns
Arrangement Track Controls
Arrangement Track Options
Arrangement Volume
Assign Icon
Audio Engine On
Auto Map
Auto Map All
Auto Quantization
Automation Fitting
Automation Mode
Backup Docs
Backup Docs Reveal In Finder
Backup Docs Show In Browser
Backup Docs Show In Explorer
Backup Docs Show In Folder
Bounce Group In Place
Bounce Group In Place Plural
Bounce Group To New Track
Bounce Group To New Track Plural
Bounce To New Track
Bounce To New Track Plural
Bounce Track In Place
Bounce Track In Place Plural
Bounce Tracks And Groups In Place
Bounce Tracks And Groups To New Track
Branch Auto Colored
Browser History Back
Browser History Forward
Cancel Pack Download
Cancel Pack Download Multi
Cancel Scene Launch
Capture Midi
Chase Notes
Chop Notes
Clean Recent
Clear All Envelopes
Clear Device Envelopes
Clear Envelope
Clear Recent
Clear Time Signature Changes
Clip Detail View Layout Auto
Clip Detail View Layout Horizontally
Clip Detail View Layout Vertically
Clip Envelope Name
Clip Markers
Close
Cloud Sign Out
Cloud Sign Out With Email
Collapse All Tracks
Color Clear All
Command Disabled
Compare Copy
Compare Switch
Complete Fragmentary Bar
Configure Ableton Link
Configure Audio Preferences
Consolidate
Consolidate To New Scene
Content Editor
Continue Playback
Convert Drums To New Midi Track
Convert Harmony To New Midi Track
Convert Melody To New Midi Track
Convert To Drum Sampler
Convert To Sampler
Convert To Simpler
Copy
Copy From Take Lane To Content Lane
Copy Parameter Name
Copy Value To Siblings
Cpu Percent Average
Cpu Percent Current
Create Branch
Create Return Branch
Cross Fade
Cut
Cut Env
Deactivate Loop
Delete
Delete All Take Lanes
Delete Fade
Delete Fragmentary Bar Time
Delete Mapping
Delete Time
Delete Unused Take Lanes
Disable All Sends
Disable Clips
Disable Clips Plural
Disable Devices
Disable Devices Plural
Disable Notes
Disable Notes Plural
Disable Scene Tempo
Disable Scene Time Signature
Disable Selection
Disable Sends
Disable Tracks
Disable Tracks Plural
Disarm Record In Arrangement
Display Generic Value
Distribute Ranges Around Root Key
Distribute Ranges Equally
Draw Mode
Dry Wet
Duplicate
Duplicate Loop
Duplicate Time
Duplicate Unlinked Loop
Edit
Edit Info Text
Edit Key Map
Edit Launch Tempo
Edit Launch Time Signature
Edit Macro Map
Edit Midi Map
Edit Plugin Program
Edit Quick Tags
Edit Time Signature
Enable All Sends
Enable Automation
Enable Browser Pre Hear
Enable Clips
Enable Clips Plural
Enable Cpu Meter
Enable Devices
Enable Devices Plural
Enable Disable Clips
Enable Disable Devices
Enable Disable Notes
Enable Disable Tracks
Enable Midi Keyboard
Enable Midi Prelisten
Enable Notes
Enable Notes Plural
Enable Sends
Enable Tracks
Enable Tracks Plural
Entry Mpe Enabled
Entry Mpe Settings
Entry Show Zoom As Db
Envelope Editor
Exclude Macro From Randomization
Exclude Macro From Snapshots
Expand All Tracks
Expand Clip Detail View
Export Midi Clip
Expression Editor
Extensions
External Sync
Extract Chains
Extract Groove
Extract Grooves
Extract Return Chains
Find
Fit Content To View Height
Fit Content To View Width
Flip Min Max Value
Flip Min Max Values
Focus Arrangement
Focus Browser
Focus Clip Editor
Focus Clip Panels
Focus Device Chain
Focus Groove Pool
Focus Mixer
Focus Next
Focus Next Neighbor
Focus Previous
Focus Previous Neighbor
Focus Session
Focus Transport
Focus Tuning
Fold
Fold Selected Track
Fold Selected Tracks
Fold To Notes
Fold To Scale
Follow
Freeze
Freeze Plural
Full Height Browser
Fullscreen
Fx Slots
Goto Current Project
Goto Library
Goto Next Clip Detail View Tab
Goto Os Desktop
Goto Root Mac
Goto Root Pc
Goto User Home Mac
Goto User Home Pc
Grid Adaptive
Grid Bigger Tuplet
Grid Fixed
Grid Narrow
Grid Off
Grid Quintuplet
Grid Septuplet
Grid Set Medium
Grid Set Narrow
Grid Set Narrowest
Grid Set Wide
Grid Set Widest
Grid Smaller Tuplet
Grid Triplet
Grid Tuplet Off
Grid Wide
Group Device Next Similar
Group Device Previous Similar
Group Device Replace Drum Samplers With Simplers
Group Device Replace Simplers With Drum Samplers
Group Device Return To Reference
Group Device Update Similar Sample Reference
Group Note Probabilities
Group Selection
Hide All Extra Columns
Hide Clip And Device Views
Hide Clip View
Hide Device View
Hide From Sidebar
Hide Grouped Tracks
Hide Lane
Hide Midi Note Filters
Hide Scale Controls
Hide Selected Grouped Track
Hide Selected Grouped Tracks
Ignore Tuning System
Import Audio File
Import File
Import Midi File
In Out
Info
Inline Rack Mixer Hide Children
Inline Rack Mixer Hide Children Nested
Inline Rack Mixer Show Children
Inline Rack Mixer Show Children Nested
Insert Captured Scene
Insert Cue
Insert Fade
Insert Midi Clip
Insert Midi Clips
Insert Midi Track
Insert Send
Insert Shape
Insert Special
Insert Take Lane
Insert Time
Insert Time Signature Change
Insert Track
Inspect One Project
Install Downloaded Pack
Install Downloaded Pack Multi
Install Package
Invert Selection
Join Notes
Latency Compensation
Latency Compensation Play Through
Launch Quantisation Enable
Launch Quantisation Narrower
Launch Quantisation Triplet
Launch Quantisation Wider
Legacy Hi Q Mode
Legacy Smoothing
Link Tracks
Load Demo Set
Locate Folder
Locate Pack
Locate Patch File
Lock Envelope
Loop Current Region
Loop Selection
Loop Selection Or Activate Deactivate Loop
Loop To Next Cue
Loop To Next Time Signature Change
Mac About
Mac Emoji And Symbols
Mac Help
Mac Hide
Mac Hide Others
Mac Preferences
Mac Quit
Mac Services
Mac Show All
Manage Projects
Map To
Map To All Siblings
Max For Live Developer Mode
Max For Live Window
Metronome While Recording
Mid Side Balance
Midi Editor
Mixer
Mixer Controls
Mono Side Chain
Move Insert Marker To Playhead Position
Move Pack
Move Packs
Navigate To Crash Report
Navigate To File Manager
Navigate To Help View
Navigate To Learn View
Navigate To Pack Overview
Navigate To Undo History
New
Next Similar
No Commands Available
Num Voices Env Time Control
Number Entry
Number Entry Create
Open
Open Pack Web Page
Open Recent
Over View
Paste
Paste Bounced Audio
Paste Time
Pause Pack Download
Pause Pack Download Multi
Performance Impact
Pfl Switches
Pitch Bend Range
Play
Play Arrangement Selection
Play By Key
Play From Insert Marker
Playback Options Name
Plugin Windows
Pref
Pref Fade Button
Pref Record On Launch
Pref Scene Tempo And Time Signature
Pref Select Next Scene On Launch
Previous Similar
Probability Group Play All Type
Probability Group Play One Type
Purchase License
Q Eigth
Q Eigth And Eigth Triplets
Q Eigth Triplets
Q No
Q Quarter
Q Sixteenth
Q Sixteenth And Sixteenth Triplets
Q Sixteenth Triplets
Q Thirtysecond
Quantize
Quantize Settings
Quit
Record In Arrangement
Record In Session
Record Monitoring Latency
Redo
Remote Content Unpair
Remove From Sidebar
Remove Macro Mapping
Rename
Rename Device
Render To Disk
Render To Disk No Video
Reset Bank To Default
Reset Color
Reset Fades
Reset Labels Order
Reset Nonautomated Midi Controllers On Clip Starts
Reset To Default Icon
Restore Clip And Device Views
Restore Clip View
Restore Device View
Resume Pack Download
Resume Pack Download Multi
Retune Set On Tuning System Change
Return Play Position
Return To Reference
Returns
Reveal In Finder
Ruler Menu Name
Ruler Normal
Ruler Smpte24
Ruler Smpte25
Ruler Smpte29
Ruler Smpte30 Drop
Ruler Smpte30 Non Drop
Sample Editor
Save
Save A Copy
Save As
Save As Default
Save As Default Set
Save As Template
Save Self Contained
Scale Highlighting
Scene Tempo And Time Signature
Select All
Select Content
Select Loop
Select On Launch
Select Scene Content
Select Stereo Pan Mode
Select Stereo Split Pan Mode
Select To Next Cue
Select To Next Time Signature Change
Sends
Separate Stems For Time Selection
Separate Stems To New Audio Tracks
Separate Stems To New Tracks
Set111 Here
Set All Clips And Grouped Tracks To Track Color
Set All Clips To Track Color
Set Clip End
Set Clip Start
Set Loop End
Set Loop Start
Set Song Start Time
Show All Envelopes
Show All Parameters
Show All Tracks
Show Automated Envelopes Only
Show Automated Parameters Only
Show Automation
Show Automation In New Lane
Show Branch Names In Editor
Show Browser
Show Clip View
Show Controlling Device
Show Cpu Percent
Show Device View
Show File Extensions
Show Groove Pool
Show Grouped Tracks
Show Hide Overload Indicator
Show In Browser
Show In Explorer
Show In Folder
Show In Places
Show Keyword In Editor
Show Lane
Show Midi Note Filters
Show Mod Matrix
Show Modulation Source
Show Old Version
Show Pack Info
Show Patch In Browser
Show Scene View
Show Selected Grouped Track
Show Selected Grouped Tracks
Show Similar Files
Show Take Lanes
Show Tuning Pool
Show Velocity Off
Show Velocity On
Slice To New Midi Track
Snap Automation To Grid
Snap To16th Grid
Snap To Grid
Solo
Solo In Place
Solo Plural
Solo Switches
Sort Branches
Sort Parts By Key Ascending
Sort Parts By Key Descending
Sort Parts By Name Ascending
Sort Parts By Name Descending
Sort Parts By Selector Ascending
Sort Parts By Selector Descending
Sort Parts By Velocity Ascending
Sort Parts By Velocity Descending
Span
Split
Split Clip From Within
Split Notes
Start Pack Download
Start Pack Download Multi
Start Transport With Record
Start Transport With Tap Tempo
Step Input Mode
Stop
Strip
Swap Arrangement Session
Swap Detail
Swap Lane
Swap Next Similar Sample
Swap Previous Similar Sample
Switch To Audio Content Tab
Switch To Envelope Tab
Switch To Expression Tab
Switch To Midi Piano Roll Content Tab
Switch To Next Mode
Switch To Previous Mode
Text Add Clip Stop
Text Add Cue
Text Add Time Signature Change
Text Auto Spelling Option
Text Avoid Freezing Onsets
Text Both Spelling Option
Text Browse Library
Text Cant Re Do
Text Cant Un Do
Text Chain Selector Filter Midi Ctrl
Text Check For Updates
Text Clear All Envelopes
Text Clear Envelope
Text Clear Envelopes
Text Clear Slices
Text Clear Time Signature Changes
Text Copy A To B
Text Copy B To A
Text Copy Band
Text Copy Envelope
Text Copy Lom Path
Text Copy Oscillator
Text Copy Oscillator To All
Text Copy Time
Text Count In
Text Create Clip In New Track
Text Create Follow Action Chain
Text Create Sub Keyword
Text Crop
Text Crop Plural
Text Crop Sample
Text Crop Sample To Time Selection
Text Crop Samples
Text Crop To Time Selection
Text Crossfade
Text Cut Time
Text Default Crossfade
Text Delete Automation
Text Delete Cue
Text Delete Keyword
Text Delete Onsets
Text Delete Time Signature Change
Text Draw Tool All
Text Draw Tool Even
Text Draw Tool Odd
Text Edit In Max
Text Enable Auto Tags
Text Enable Only While Recording
Text Enharmonic Spelling
Text Export As Ams
Text Export To Abl
Text Fade
Text Fade In
Text Fade Out
Text Fit To Time Range
Text Flat Response
Text Flat Spelling Option
Text Fold
Text Group
Text Group By Creator
Text Group By Filter
Text Group To Drum Rack
Text Help About
Text Help Authorizations And Trial
Text Help Community Ableton
Text Help Home Page Ableton
Text Help Manual Ableton
Text Help Support Ableton
Text Hi Quality
Text Hide All Filter Groups
Text Hide Filter
Text Insert Onsets
Text Insert Scene
Text Insert Silence
Text Insert Warp Marker
Text Legato
Text Live8 Legacy Key Mod
Text Live8 Legacy Mode
Text Lock
Text Loop On Off
Text Loop Snap
Text Loop Zoom
Text Manage Patch
Text Marker Snap
Text Marker Zoom In
Text Metronome Presets
Text Midi Note Number Spelling
Text Normalize
Text Normalize Panorama
Text Normalize Panoramas
Text Normalize Sample File
Text Normalize Volume
Text Normalize Volumes
Text Per Note Pitch Bend
Text Pitch Lock Off
Text Pitch Lock On
Text Pitch To Env Modulation
Text Play Duplicate Notes When Strumming
Text Pre Dc Filter
Text Re Do
Text Reload Loops From File
Text Rename Keyword
Text Render All Individual Tracks
Text Render Selected Tracks
Text Reset All Gains
Text Reset Filter Groups To Default
Text Reset Onsets
Text Reset Slices
Text Reset Tag Order
Text Reset To Default Value
Text Reveal Sample
Text Reverse
Text Reverse Sample
Text Root Note Spelling
Text Sample Manage
Text Samples
Text Save As Default Configuration
Text Save As Default Preset
Text Scale Non Dpi Aware Plugin
Text Seconds
Text Select All With Same Range
Text Send Per Note Events To Generated Notes
Text Set Envelope Levels Max
Text Set Envelope Levels Mid
Text Set Envelope Levels Min
Text Set Length
Text Set Morph To Bp
Text Set Morph To Hp
Text Set Morph To Lp
Text Set Morph To No
Text Sharp Spelling Option
Text Shelf Scale Legacy Mode
Text Show All Filter Groups
Text Show Max Window
Text Show Note Values For Filter Frequency
Text Show Preset Name In Title Bar
Text Show Quick Tags
Text Show Splas Screen
Text Show Tag Editor
Text Slice To Drum Rack
Text Slice To New Midi Track
Text Split
Text Sub Clip Stop
Text Tick Duration
Text Tune Pitch Bend
Text Un Do
Text Unfold
Text Use Const Pow Crossfade
Text Use System
Text Use System With Device
Text Zero Latency Dry
Text Zoom In Played
Text Zoom In Release Loop
Text Zoom In Sustain Loop
Text Zoom Out
Title Edit
Title File
Title Help
Title Insert
Title Navigate
Title Options
Title Playback
Title View
Toggle Arrangement Session
Toggle Audition
Toggle Detail
Toggle Dual Window Mode
Toggle Learn View
Toggle Metronome
Track Options
Undo
Unfold All Tracks
Unfold Selected Track
Unfold Selected Tracks
Unfollow Tempo Automation
Unfreeze
Unfreeze Plural
Ungroup
Ungroup Note Probabilities
Uninstall Live Pack
Unlink Tracks
Unmap From
Unsolo
Unsolo Plural
Update Browser
Update Similar Sample Reference
Use Tab To Navigate Controls
Video Window
Volume
Warp From Here
Warp From Here Start At Song Tempo
Warp From Here Straight
Warp From Here Straight With Song Tempo
Warp Sample As Loop In Bars
Warp Sample As Loop In Beats
Warp Selection As Loop In Bars
Warp Selection As Loop In Beats
Window Zoom
Wrap Keyboard Navigation
Zoom Back From Clip Time Selection
Zoom Back Out Time
Zoom From Arrangement Time Selection
Zoom In Time
Zoom In Vertically
Zoom In Window
Zoom Large
Zoom Medium
Zoom Out Clip Completely
Zoom Out Time
Zoom Out Time Completely
Zoom Out Vertically
Zoom Out Window
Zoom Selected Arrangement Lanes
Zoom Small
Zoom To Arrangement Time Selection
Zoom To Clip Time Selection
Zoom To Fit
Zoom Window To Default
```

## Dialoger — 523

Alla `sDialog*`-symboler.

```
Abort Render For Audio Device Change
About Re Wire Device Mode1
About Re Wire Device Mode2
Account Button
Add Folder
Analyzing Database
Ask Restore
Ask Restore After Crash During Last Restore
Audio Settings
Auth Product Info Plus Add Ons
Authorization Successful
Authorizing Products
Average Bit Rate
Bounce
Bounce Finished
Bounce Push
Bouncing Arrangement
Bouncing Arrangement Realtime
Bouncing Session View Clips
Bouncing Session View Clips Realtime
Bug Report Packages Cannot Be Installed
Can Impair Audio
Cancel
Cancel Button Text
Cannot Authorize Saving Disabled
Cannot Authorize Within Time Limit
Change Dpi Scaling Setting
Changes Applied When Restarted
Changes Will Be Lost
Check Connection
Check Installation Size
Check Package Dependencies
Choose File
Choose Package File
Chose Collect And Save Options
Click Below To Authorize
Click Below To Get14 Day License
Collect And Save
Collect And Save Continue Transfer
Collect And Save Stop Transfer
Collect Into Library
Collect Into Project
Column Candidates
Column Location Type
Column Name
Column Path
Column Project
Combined License Authorization Deactivated
Combined License Cannot Authorize Saving Disabled
Combined License Expired
Combined License Expired On Startup
Committing
Compare Products
Confirm Delete Keyword
Confirm Delete Keyword Group
Confirm Delete Keyword Group In Label Spec
Confirm Delete Keyword Group In Label Spec Recursive
Confirm Delete Keyword Group Recursive
Confirm Delete Keyword In Label Spec
Confirm Delete Keyword In Label Spec Recursive
Confirm Delete Keyword Recursive
Confirm Reset Tag Order
Contacting Server
Contains Downloading Sample
Contains Downloading Samples
Content Clips
Content Defects
Content Media Files
Content One Clip
Content One Defect
Content One Media File
Content One Preset
Content One Set
Content Presets
Content Sets
Control Back
Copy Media Files Multiple Documents Too
Copying
Copying Media Files
Could Not Save Pad Default
Could Not Save Track Default
Create New Candidate Group
Create New Candidate Parent
Create New Candidate Tag
Create New Folder
Create New Folder Label
Current Document
Current Document And Project
Current Document Externals Summary
Current Document Missing Media Files
Current Document No Externals But Missing Summary
Current Document No Externals Summary
Current Document No Missing Media Files
Current Document Not Yet Named
Current Document One Missing Media File
Custom Settings
Default Preset
Delete File Or Folder
Delete File Or Folder Title
Delete Remote Content File Confirmation Button
Delete Selected File
Delete Selected Files
Delete Selected Remote Content File
Delete Selected Remote Content Files
Demo Version Cant Save
Doc Name For Save A Copy
Document Contains Unknown Features
Drag And Drop To Install
Drop Custom Folder Here
Eas Institution Name Default
Eas License Name Default
Edu Access Expired
Edu Access Successful Unlock
Elsewhere Media Files
Encoder Settings
Error Message Template
Estimated File Size
Explain Library
Extension Confirm Install
Extension Install Failed
Extension Install Timed Out
Extension Installed Message
Extension Installed Title
Extension Uninstall Failed
Extension Uninstalled Message
Extension Uninstalled Title
External Elsewhere Media Files
External Library Media Files
External Media Files In Current Document Section
External Media Files Section
External Project Media Files
Failed To Get Authorization
Fetch
File List Text
Fix All Options Section
Frame Rate
Freeze
Freeze Finished
Freeze Push
Freezeing Arrangement
Freezeing Arrangement Realtime
Freezeing Session View Clips
Freezeing Session View Clips Realtime
Hardware Code File Content
Hardware Code File Name
Height
Import File
Import Midi Meta Data Message
Initial Message
Install Package
Installation Finished Incomplete
Installation Finished Incomplete Need Unlock
Installation Finished Success
Installation Finished Success Need Unlock
Installation Prompt
Installed Re Wire Engine
Installing
Installing Legacy Packs
Integrating
Interface Error Learn More
Internet Connection Required For Trial
Intro
Keep Unstretched Warp Markers
Key Learn1
Key Learn2
Legacy Api Warning Max
Legacy Api Warning Script
Less Than One
Library Contents Section
Library Contents Summary
Library Externals Summary
Library Media Files
Library Missing Media Files
Library No Externals But Missing Summary
Library No Externals Summary
Library One Missing Media File
License Expired On Startup
License Id Missing
Licenses
List Of Changed Documents
List Of Clips
List Of Defects
List Of External Media Files
List Of Grooves
List Of Media Files
List Of Media Files From Library
List Of Media Files From Other Projects
List Of Presets
List Of ROM Media Files From Library
List Of Sets
List Of Unused Media Files From Elsewhere
List Of Unused Samples From Bounce
List Of Unused Samples From Consolidate
List Of Unused Samples From Freeze
List Of Unused Samples From Recording
Load Theme
Locate Pack Prompt
Locate Sample
Location Candidate
Location Candidates
Location Downloading
Location External
Location Factory Content
Location Library
Location Missing
Location Other Project
Location Section
Location This Project
Looking In
Merging Stems
Midi Map Mode Conflict 14 Invalid
Missing Media Files
Missing Media Files Section
Mode Dark
Mode Follow System
Mode Light
Move Pack Prompt
Move Packs Behaviour
Move Packs Prompt
Moving
Multiple Candidates
Nesting Level Too Deep
New Language Selected
No Auth Product Info No Add Ons
No Auth Product Info Plus Add On
No Candidates Available
No Candidates Message
No Current Project
No Library
No Missing Media Files
No Move In Demo
Ok
Ok Button Text
One External Elsewhere Media File
One External Library Media File
One External Project Media File
One ROM Media File
One Unused Bounce Sample
One Unused Consolidate Sample
One Unused Elsewhere Media File
One Unused Freeze Sample
One Unused Recording Sample
Open A Python File
Open Document
Open License Prefs
Open Max Preferences
Optimizing Database Title
Out Of Demo Mode Creating New Set Proceed
Out Of Demo Mode Creating New Set Save Now
Overwrite Default Audio Track
Overwrite Default Midi Track
Overwrite Default Pad Preset
Overwrite Default Preset
Overwrite Existing File
Overwrite Plugin Parameter Defaults
Package Newer Version
Packing Finished Incomplete
Packing Finished Success
Packing Section
Plugins Could Not Be Loaded
Prepare Installation
Prepare Packing For Installation
Preparing Installation
Presets
Process Dpi Awareness Changed
Progress Slicing
Project Contents Section
Project Contents Summary
Project Create Pack
Project Externals Summary
Project Externals Warning
Project Is From Old Version
Project Is Legacy
Project Missing Media Files
Project No Externals But Missing Summary
Project No Externals Summary
Project No Missing Media Files
Project One Missing Media File
Project Unused Summary
Project Unused Summary Nothing
Project Unused Summary One Media File
Project Unused Warning
Projects Media Files
Purchase Now Button
ROM Media Files
Re View Media Files Section
Reading Package Information
Register On Web Page
Render Choose Dir
Render Save Audio File As
Rent To Own Cannot Authorize Saving Disabled
Rent To Own Expired
Rent To Own License Expired On Startup
Repair Successful Please Save
Replace Or Edit Media Files
Rescan Files
Rescan Files Status
Result Not Ok
Result Ok
Retrieving License Information
Retry Button
Rewrite Legacy Files Status
Rewrite Legacy Files Title
Running As Re Wire Device
Running As Rewire Device
Sample Rate
Save A Copy
Save Abl Preset As
Save Changes
Save Changes Before Proceeding
Save Clip
Save Document As
Save Hardware Code Prompt
Save Self Contained
Save Theme
Save With No Project
Saving And Exporting Are Disabled
Saving And Exporting Are Disabled And Can Be Activated In Prefs
Saving And Exporting Will Be Disabled
Scanning
Search For Group Tag
Search In Current Project
Search In Custom Folder
Search In Library
Select Extension File
Select Sample Editor
Select Vst3 Directory Prompt
Select Vst Directory Prompt
Self Containing
Separating Bass
Separating Drums
Separating Other
Separating Stems
Separating Vocals
Session Grouped Loop Length Limit Entry
Setting Up Library Rescan
Show Scrollbars Always
Show Scrollbars Follow System
Show Scrollbars When Scrolling
Some Media Files Missing
Some Products Could Not Be Unlocked
Some Products Have Max Unlock Count
Splice Downloading
Splice Update Is Available
Splice Update Paused
Stay In Demo Mode For Current Set
Successfully Unlocked
Sync Disabled
Tag Must Not Be Duplicate
Tag Must Not Be Empty
Tag Must Not Contain Delimiter
Tell Me More
Text Audio Device
Text Authorize Product
Text Authorized For
Text Auto Update Available
Text Auto Update Continue Button
Text Auto Update Prefs Button
Text Auto Update Remind Me Later Button
Text Beta Release Notes Url
Text Buffer Size
Text Cant Delete Temp Files
Text Cant Save To Read Only Dir
Text Channel Config In
Text Channel Config Out
Text Channel Warning
Text Choose Custom Search Folder
Text Choose Mx D App
Text Choose Mx D App Folder
Text Clip Conversion Quality
Text Clock Source
Text Configuration
Text Consolidating
Text Delay
Text Delete Temp Files Directory
Text Device
Text Device Change Detected
Text Device Type
Text Driver Latency Error
Text Drop Clips Or Grooves Here
Text Drop Live Clips Here
Text Dump Push Warning
Text Dump Warning
Text Exporting
Text Falling Back To Bundled Max App
Text Hardware Setup
Text Index Update
Text Index Update Counted
Text Input Buffer Size
Text Input Configuration
Text Input Device
Text Input Devices
Text Input Latency
Text Input Ports
Text Installing Vcredist
Text Latencies Section
Text Legal Information
Text Link
Text Link Audio
Text Link Audio Alert
Text Link Audio Buffered
Text Link Audio Enable Link Audio For Peers
Text Link Audio Enable Link For Peers
Text Link Audio Latency
Text Link Audio Name
Text Link Audio No Peers
Text Link Audio Peers
Text Link Audio Section
Text Link Audio Status
Text Link Audio Status Has Dropouts
Text Link Audio Status Is Available
Text Link Audio Status Is Connected
Text Link Audio Sync To Incoming Audio
Text Link Driver Alert
Text Link Section
Text Link Start Stop Sync
Text Midi Clock Mode
Text Midi Devices Require Restart
Text Midi Syncronization
Text Mono Inputs Column Title
Text Mono Outputs Column Title
Text More Than One Push
Text Move Install Audio Driver
Text Mpe
Text No Lessons
Text Not Authorized
Text Not Set
Text Offset
Text Output Buffer Size
Text Output Configuration
Text Output Device
Text Output Devices
Text Output Latency
Text Output Ports
Text Overall Latency
Text Patch Already Editing
Text Path Missing
Text Push Windows Driver Warning Message
Text Rate
Text Re Wire Device Sample Rate
Text Red Hint Cpu Overload
Text Red Hint No Engine
Text Red Hint Select Input
Text Red Hint Select Output
Text Release Notes
Text Release Notes Url
Text Remote
Text Remote Script
Text Render To Disk
Text Render To Disk No Video
Text Rendering
Text Requires Firmware
Text Resync Ext Hardware Button Show
Text Resync Ext Hardware Section
Text Resync Style
Text Rewire Device Settings
Text Sample Rate
Text Sample Rate Section
Text Save Patch As
Text Serial Number
Text Setting Temp Folder Doesnt Move Current Temp Folder
Text Simple Update Finished
Text Stereo Inputs Column Title
Text Stereo Outputs Column Title
Text Stop Playing
Text Switch Sync Off And Stop Playing
Text Sync
Text System Device Changed Engine Deactivated
Text Tempo Follower Input Channel
Text Tempo Follower Section
Text Tempo Follower Show
Text Test Entry
Text Title
Text Track
Text Trial Authorized For
Text Tuning System Modify Midi Clips
Text Type
Text Update Available
Text Updating Error
Text Updating Firmware
Text Using Bundled Mx D App
Text Vcredist Needs Installation
Theme Default
Time Left
Timeable Fine Control Suffix
Title
Title Current Document
Title Library
Title Missing
Title Preferences
Title Project
Title Replace Media Files
Title Separate Stems
Title Undo History
Tone Cool
Tone Neutral
Tone Warm
Trial Authorized
Trial Expired
Trial Is Now Expired
Trial Period In Days
Trial Period In Hours
Trial Period Less Than
Try To Repair All Broken Media Files Like This
Uninstall Media Files From Previous Packages
Unlock New Packages
Unused Bounce Samples
Unused Consolidate Samples
Unused Elsewhere Media Files
Unused Freeze Samples
Unused Media Files Section
Unused Recording Samples
Update Package
Upload
Upload Conflict
Upload Keep Both
Upload Overwrite
Upload Stop Transfer
Video Settings
Warn About Dangers Of Running Untrusted Code From The Internet
Why
Width
Working
```

## Statusraden — 200

Alla `sStatus*`-symboler.

```
Accent
Add Audio Track
Add Midi Track
Adding Item
Analysing
Analysing File
Analysing Sample
Analysis Decoding
Analysis Progress
Analysis Status View Text Analysis Done
Analysis Status View Text Analysis Paused
Analysis Status View Text Analyzing Files
Analysis Status View Text Analyzing Files With Count
Analysis Status View Text Indexing Files
Analysis Status View Text Indexing Files With Count
Analysis Status View Text Scanning Files
Analysis Status View Text Scanning Files With Count
Analyzing File
Analyzing For Normalization
Arranger Preview
Awaiting Centered Pick Up
Awaiting Pick Up Infix
Bar Cloud Multiple Songs Are Downloading
Bar Cloud Song Is Downloading
Bar Cloud User Library Unavailable
Bar Link Audio Channel Info
Bar Link Audio Peer Info
Bar Link Info
Bar Link Info Audio Status
Bar Link Info Audio Status Available
Bar Link Info Audio Status Connected
Bar Link Info Audio Status Dropouts
Bit Reduction No Dithering
Bit Reduction Only Converting
Bit Reduction With Dithering
Buffering Samples
Cannot Display Specific Value
Cannot Insert Device
Cloud Needs Sign In Text
Collecting Documents
Control Unassigned
Control Unassigned Macro
Copying File
Corrupt File Found
Deleting File
Diabled Reason No Sends
Disabled Because Macro Controllee
Disabled Because Parameter Does Not Exist In Plugin
Drop Audio Effects
Drop Effect Or Instrument
Drop Midi Effects
Drop Packages To Install
Drop Position Text
Drop To Import
Drop To Install
Duration Text
Encoding
End Text
File Analysing Text
File Copied
File Copying
File Downloading Text
File Offline Text
Filter Selected
Filter Selected Plural
Finished
Found Files
Frozen Tracks In Multi Selection
Importing
Importing Preset
Importing Sample
Input Key Controller
Input Key Controller None
Insert Mark Text
Installing Item
Invalid Controller Layout
Invalid Position For End Marker
Invalid Position For Loop End
Invalid Position For Start Marker
Learn Midi Note Range Text
Length Text
Loading Max
Locked To Infix
Message Current Tuning
Midi Controller
Midi Key Tuning Table Note
Midi Macro Mapping
Midi Map Mode
Midi Note
Midi Note Range
Midi Pitch Bend
Multiple Audio Clips
Multiple Audio Clips Different Length
Multiple Audio Clips Different Warp Markers
Multiple Audio Clips Single Track
Multiple Audio Clips Single Track Different Length
Multiple Audio Clips Single Track Different Warp Markers
Multiple Clips
Multiple Midi Clips
Multiple Midi Clips Header
Multiple Midi Clips Max Exceeded
Multiple Midi Clips No Sync
Multiple Midi Clips Single Track
Multiple Midi Clips Unlooped
Multiple Midi Tracks Max Exceeded
No Automation Events Could Be Removed
No Clip Selected Text
Normalizing
Normalizing File
Octave
Over Midi Key And Target Text
Over Midi Key Frequency
Over Midi Key Note
Parameter Controlled By Device
Pitch Class And Octave
Plugins Could Not Be Loaded
Prepare Audio
Preparing
Preparing Cropped File
Preparing Reverse File
Re Writing Document
Reading Attributes
Redoing
Render Audio
Render Audio Realtime
Replace Drum Samplers With Simplers
Replace Simplers With Drum Samplers
Resampling
Reversing File
Samples To Open
Selecting File
Send Disabled By Zooming
Setup
Splice Sample Invalid
Start Text
Starting Editor
Tempo Disabled Because Of Tempo Follower
Tempo Disabled Because Of Tempo Leader Clip
Text Bank Loaded
Text Corrupt File Found
Text Creating Plugin
Text Falling Back To Bundled Max App
Text Flip Clips
Text Missing Files
Text Must Reauthorize To See Pack Downloads
Text No Audio Activation
Text Note Disabled
Text Note Group
Text Note Pitch
Text Note Probability
Text Note Selection
Text Note Time
Text Note Velocity
Text Note Velocity Range
Text Plugin Config Mismatch
Text Recording Disc Space
Text Scan Au
Text Scan Plugins
Text Scan Plugins With Details
Text Scan Plugins With Details And Name
Text Scan Vst2 Plugins
Text Scan Vst3 Plugins
Text Session Is Playing
Text Session Is Playing Main Track
Time Range Text
Time Text
Undoing
Undoing Redoing Num Processed Num Total Maneuver Name
Unknown Au Plugin
Unknown Vst2 Plugin
Unknown Vst3 Plugin
Unlocked From Infix
Unsuable Preset
Unsupported Scale
Unsupported Scale In Bank
Unsupported Tuning
Unsupported Tuning In Bank
Upgrade Prevented While Recording
Video Render Progress
View Begin Initialize Midi
View Create A New Document
View Create Bug Report
View Init Application
View Init Audio
View Load Song Document
View Load Template Song Document
View Loading
View Restoring File From Undo History
View Restoring File From Undo History Progress
View Saving
View Saving A Copy
View Saving As
Warp Marker Segment BPM Limit Reached
Warping
Window Zoom
Writing
Writing Cropped File
Zoom Scroll
Zoomed Key Range
Zoomed Selection
```

## Ledtrådar (tooltips) — 2302

Alla `sInfoText*`-symboler.

```
Accidental Spelling Auto
Accidental Spelling Flats
Accidental Spelling Sharps
Accidental Spelling Sharps And Flats
Add Cue
Add Lane
Additional Automation Lane
Algorithm
Alsa Buffer Size
Alsa In Selected Device
Alsa Out Selected Device
Amount
Anti Alias
Arranger
Arranger Recording
Asio Buffer Size
Attack
Audio
Audio Content
Audio From
Audio To
Audio To Midi Dialog
Audio Tools
Audio Track
Audio Track With Parameter
Audio Track Without Parameter
Audio Transformation Tools
Audition Button
Auto Map
Auto Map All
Auto Select
Auto Update Continue
Auto Update Downloading
Auto Update Prefs
Auto Update Ready
Auto Update Remind Me Later
Auto Update Vcredist Continue
Auto Update Version
Automated Envelope Chooser
Automated Parameter Chooser
Automation Fitting
Automation Modulation Toggle
Aux Env On Off
Aux Env Section
Back
Background
Bandwidth
Beat Repeat
Bias
Bpm
Branch Auto Colored
Branches List
Browse
Bypass
Cancel Scene Fire
Candidates
Chain Selector Filter Midi Ctrl
Chance
Changes Applied On Restart
Channel
Channel Label
Choose Scene
Chord Selector
Clear Button
Clear Slices
Clip
Clip Conversion Quality
Clock Source
Close File Manager
Close Help View
Close Undo History
Coarse Input
Coarse Output
Color Indicators
Column Grid
Column Name
Column Quantization
Column Random
Column Timing
Column Velocity
Command Mac
Command Pc
Commit Groove
Commit Pending Changes
Common
Compand
Compare Copy
Compare Switch
Compare Switched To Preset
Compare Toggled Preset
Configuration
Configure Mode
Context Menu Mac
Context Menu Pc
Copy From Take Lane To Content Lane
Copy Lom Path
Copy Mac
Copy Pc
Core Audio Buffer Size
Core Audio In Selected Device
Core Audio Out Selected Device
Cpu Load Meter
Crash Report
Create Clip In New Track
Create Keyword Group
Create New Keyword
Create Pack
Create Sub Keyword
Crop Sample
Cross Fade Switch
Cross Fade Volume
Crossfade Equal
Crossfade Left
Crossfade Panel
Crossfade Range
Crossfade Right
Cue Marker
Cue Out
Curve
Curve Handle
Custom Editor
Custom Icon
Default
Default Chain Name Suffix
Default Groove
Default Name Suffix
Default Track Name Suffix
Delete Keyword
Destination
Device
Device Chooser
Device Editing
Device Type
Device View
Disabled Because Following Scale
Disabled Because Midi Mode
Disabled Because Tuning System
Disabled Because Unsupported Track
Disabled Similar Files
Display
Distribute Ranges Around Root Key
Distribute Ranges Equally
Download Splice Sample
Draw Mode
Drive
Drum Branch Trigger Button
Dry Wet
Edit Group
Edit Mode
Edit Mode Without Deviations
Edit Patch
Edit Sample
Editable Palette Button Button
Editable Palette Button Name Edit Field
Editing Disabled
Enable Auto Tags
Enable Follow Actions
Enable Label
Enlarge
Env Attack
Env Attack Level
Env Attack Slope
Env Decay
Env Decay Level
Env Decay Slope
Env Display
Env Loop Time
Env Mode
Env Release
Env Release Level
Env Release Slope
Env Retrigger Rate
Env Sustain Level
Env Vel Scale
Envelope
Envelope Chooser
Envelope Display
Envelope Section
Envelopes Editor View Mode
Expression Lane Header
Extended Clip Properties
Extract Chains
Fade In Handle
Fade In Start Handle
Fade Out End Handle
Fade Out Handle
File Manager
Filter
Filter Bandpass
Filter Bandpass M1
Filter Bandpass M2
Filter Circuit Bp No Mo
Filter Circuit Lp Hp
Filter Comb
Filter Comb Inverted
Filter Comb Inverted M1
Filter Comb Inverted M2
Filter Comb M1
Filter Comb M2
Filter Cutoff
Filter Cutoff By Key
Filter Cutoff By Vel
Filter Dfm
Filter Dfm M1
Filter Dfm M2
Filter Display
Filter Drive
Filter Edit View
Filter Env Amount
Filter Env On Off
Filter Env Scale
Filter Eq Notch
Filter Eq Notch M1
Filter Eq Notch M2
Filter Eq Peak
Filter Eq Peak M1
Filter Eq Peak M2
Filter Extensions
Filter Filther
Filter Filther M1
Filter Filther M2
Filter Finger
Filter Frequency
Filter Highpass
Filter Highpass M1
Filter Highpass M2
Filter Key Scale
Filter Legacy Q
Filter Legacy Type
Filter Lowpass
Filter Lowpass M1
Filter Lowpass M2
Filter Membrane Resonator
Filter Membrane Resonator M1
Filter Membrane Resonator M2
Filter Morph
Filter Nonexistent
Filter On
Filter On Off
Filter Phaser
Filter Phaser M1
Filter Phaser M2
Filter Plate Resonator
Filter Plate Resonator M1
Filter Plate Resonator M2
Filter Redux
Filter Redux M1
Filter Redux M2
Filter Res
Filter Resonance
Filter Section
Filter Slope
Filter Svf12db
Filter Svf12db M1
Filter Svf12db M2
Filter Svf24db
Filter Svf24db M1
Filter Svf24db M2
Filter Switched Resistor Lp
Filter Switched Resistor Lp M1
Filter Switched Resistor Lp M2
Filter Type
Filter Vel Scale
Filter View
Filter Vowel
Filter Vowel M1
Filter Vowel M2
Find Media File
Fine Input
Fine Output
Finish Lesson
Fire Selected Scene
Fire Selected Track
Fit All Tracks To Screen Height
Fit Song To Screen Width
Fix All
Flip Min Max
Floating Midi Stretch Marker
Focus Mode
Fold
Fold Additional Automation Lanes
Fold To Notes
Fold To Scale
Follow Action Time
For More Packs
Forward
Fragmentary Bar Marker
Freeze Clip
Gate
Glide On
Glide Time
Global Finger
Global Groove Amount
Global Time
Grid
Grid Display
Groove
Group By Creator
Group By Filter
Group Device Next Similar
Group Device Previous Similar
Group Device Return To Reference
Group Device Update Similar Sample Reference
Group Fold
Group Stop Button
Hand
Hardware Setup
Harmonics Editor
Header
Header Accidental Spelling Auto
Header Accidental Spelling Flats
Header Accidental Spelling Sharps
Header Accidental Spelling Sharps And Flats
Header Add Cue
Header Add Lane
Header Additional Automation Lane
Header Algorithm
Header Alsa Buffer Size
Header Alsa In Selected Device
Header Alsa Out Selected Device
Header Amount
Header Annotation
Header Anti Alias
Header Arranger
Header Arranger Recording
Header Attack
Header Audio
Header Audio From
Header Audio To
Header Audio To Midi Dialog
Header Audio Tools
Header Audio Transformation Tools
Header Audition Button
Header Auto Map
Header Auto Map All
Header Auto Select
Header Auto Update
Header Automated Envelope Chooser
Header Automated Parameter Chooser
Header Automation Fitting
Header Automation Modulation Toggle
Header Aux Env On Off
Header Aux Env Section
Header Back
Header Background
Header Bandwidth
Header Beat Repeat
Header Bias
Header Bpm
Header Branch Auto Colored
Header Branches List
Header Browse
Header Buffer Size
Header Bypass
Header Cancel Scene Fire
Header Candidates
Header Chain Selector Filter Midi Ctrl
Header Chance
Header Channel
Header Channel Label
Header Choose Scene
Header Chord Selector
Header Clear Button
Header Clear Slices
Header Clip
Header Clip Conversion Quality
Header Clock Source
Header Close File Manager
Header Close Help View
Header Close Undo History
Header Coarse Input
Header Coarse Output
Header Color Indicators
Header Column Grid
Header Column Name
Header Column Quantization
Header Column Random
Header Column Timing
Header Column Velocity
Header Commit Groove
Header Commit Pending Changes
Header Common
Header Compand
Header Compare Copy
Header Compare Switch
Header Configuration
Header Configure Mode
Header Copy From Take Lane To Content Lane
Header Copy Lom Path
Header Core Audio Buffer Size
Header Core Audio In Selected Device
Header Core Audio Out Selected Device
Header Cpu Load Meter
Header Crash Report
Header Create Clip In New Track
Header Create Keyword Group
Header Create New Keyword
Header Create Pack
Header Create Sub Keyword
Header Crop Sample
Header Cross Fade Switch
Header Cross Fade Volume
Header Crossfade Equal
Header Crossfade Left
Header Crossfade Panel
Header Crossfade Range
Header Crossfade Right
Header Cue Marker
Header Cue Out
Header Curve
Header Curve Handle
Header Custom Editor
Header Custom Icon
Header Default
Header Default Groove
Header Delete Keyword
Header Destination
Header Device
Header Device Chooser
Header Device Type
Header Device View
Header Disclosure Menu Button
Header Display
Header Distribute Ranges Around Root Key
Header Distribute Ranges Equally
Header Download Splice Sample
Header Drive
Header Drum Branch Trigger Button
Header Dry Wet
Header Edit Group
Header Edit Patch
Header Edit Sample
Header Editable Palette Button Button
Header Editable Palette Button Name Edit Field
Header Enable Auto Tags
Header Enable Follow Actions
Header Enable Label
Header Enlarge
Header Env Attack
Header Env Attack Level
Header Env Attack Slope
Header Env Decay
Header Env Decay Level
Header Env Decay Slope
Header Env Display
Header Env Loop Time
Header Env Mode
Header Env Release
Header Env Release Level
Header Env Release Slope
Header Env Retrigger Rate
Header Env Sustain Level
Header Env Vel Scale
Header Envelope
Header Envelope Chooser
Header Envelope Display
Header Envelope Section
Header Envelopes Editor View Mode
Header Extended Clip Properties
Header Extract Chains
Header Fade In Handle
Header Fade In Start Handle
Header Fade Out End Handle
Header Fade Out Handle
Header File Manager
Header Filter
Header Filter Circuit Bp No Mo
Header Filter Circuit Lp Hp
Header Filter Cutoff
Header Filter Cutoff By Key
Header Filter Cutoff By Vel
Header Filter Display
Header Filter Drive
Header Filter Edit View
Header Filter Env Amount
Header Filter Env On Off
Header Filter Env Scale
Header Filter Extensions
Header Filter Finger
Header Filter Frequency
Header Filter Key Scale
Header Filter Legacy Q
Header Filter Legacy Type
Header Filter Morph
Header Filter On
Header Filter On Off
Header Filter Res
Header Filter Resonance
Header Filter Section
Header Filter Slope
Header Filter Type
Header Filter Vel Scale
Header Filter View
Header Find Media File
Header Fine Input
Header Fine Output
Header Finish Lesson
Header Fire Selected Scene
Header Fire Selected Track
Header Fit All Tracks To Screen Height
Header Fit Song To Screen Width
Header Fix All
Header Flip Min Max
Header Floating Midi Stretch Marker
Header Focus Mode
Header Fold
Header Fold Additional Automation Lanes
Header Fold To Notes
Header Fold To Scale
Header Follow Action Time
Header Forward
Header Freeze Clip
Header Gate
Header Glide On
Header Glide Time
Header Global Finger
Header Global Groove Amount
Header Global Time
Header Grid
Header Grid Display
Header Groove
Header Group By Creator
Header Group By Filter
Header Group Device Next Similar
Header Group Device Previous Similar
Header Group Device Return To Reference
Header Group Device Update Similar Sample Reference
Header Group Fold
Header Group Stop Button
Header Hand
Header Hardware Setup
Header Harmonics Editor
Header Hi Quality
Header Hi Quality Oversampling
Header Hide All Filter Groups
Header Hide Filter
Header Hide From Sidebar
Header Hide Scale Controls
Header Histogram Gr Mode
Header Histogram Out Mode
Header Home File Manager
Header Home Lessons
Header Home Pack Overview
Header Horizontal Split
Header Hot Swap
Header Hot Swap Button
Header Hot Swap Clip Groove
Header Hot Swap Groove
Header In
Header In Key
Header In Latency
Header In Out Panel
Header Include Elsewhere Media Files
Header Include External Elsewhere Media Files
Header Include External Library Media Files
Header Include External Project Media Files
Header Include Library Media Files
Header Include Project Media Files
Header Include ROM Media Files
Header Input Buffer Size
Header Input Device
Header Input Device Remote
Header Input Device Sync
Header Input Device Track
Header Input Gain
Header Input Meter
Header Insert Mark
Header Inspect Current Document
Header Inspect Current Project
Header Inspect Library
Header Interpolation
Header Interval
Header Invalid Tuning File
Header Invert Phase
Header Is Active
Header Is Current Set
Header Is In Current Set
Header Jump Next Cue
Header Jump Prev Cue
Header Key Range Button
Header Key Range Ruler
Header Key Zone Shift
Header Keyboard Conflict
Header LFO Section
Header Latency
Header Leave Hot Swap
Header Led
Header Legacy Smoothing
Header Length
Header Lessons
Header Lfo Amount
Header Lfo Amount A
Header Lfo Amount B
Header Lfo Amount Vel
Header Lfo Attack
Header Lfo Connection B
Header Lfo Display
Header Lfo Finger
Header Lfo Offset
Header Lfo On
Header Lfo On Off
Header Lfo One Filter
Header Lfo One Panorama
Header Lfo One Pitch
Header Lfo One Volume
Header Lfo Phase
Header Lfo Range
Header Lfo Rate
Header Lfo Rate Beats
Header Lfo Rate By Key
Header Lfo Rate Freq
Header Lfo Rate Key
Header Lfo Rate Type
Header Lfo Retrigger
Header Lfo Spin
Header Lfo Stereo Mode
Header Lfo To Filter
Header Lfo To Osc
Header Lfo Type
Header Lfo Waveform
Header Lfo Width
Header Link
Header Linked Track Indicator
Header List
Header Locate Media Files
Header Locate Patch File
Header Location
Header Lock
Header Loop Bar Resize
Header Loop End
Header Loop Start
Header Lower
Header MPE Editor View Mode
Header Macro Control
Header Macro Control Map Button
Header Macro Control Name
Header Macro Range Button
Header Macro Range Ruler
Header Main Automation Lane
Header Main Clip Properties
Header Main Out
Header Main Pan
Header Main Range
Header Main Stereo Split Pan L
Header Main Stereo Split Pan R
Header Main Track Header
Header Main Volume
Header Manage Sample
Header Mapping Browser
Header Menu Accessibility Speak Info Text
Header Menu Add Cue
Header Menu Add Folder To Browser
Header Menu Add Time Signature Change
Header Menu Apply Algorithm
Header Menu Arm Record In Arrangement
Header Menu Arrangement In Out
Header Menu Arrangement Overdub
Header Menu Arrangement Returns
Header Menu Arrangement Track Options
Header Menu Arrangement Volume
Header Menu Automation Mode
Header Menu Bounce To New Track
Header Menu Bounce Track In Place
Header Menu Browser History Back
Header Menu Browser History Forward
Header Menu Capture Midi
Header Menu Chase Notes
Header Menu Clean Recent
Header Menu Clear All Colors
Header Menu Clear Recent
Header Menu Clip Detail View Layout Auto
Header Menu Clip Detail View Layout Horizontally
Header Menu Clip Detail View Layout Vertically
Header Menu Close
Header Menu Consolidate
Header Menu Consolidate To New Scene
Header Menu Continue Playback
Header Menu Convert Drums To New Midi Track
Header Menu Convert Harmony To New Midi Track
Header Menu Convert Melody To New Midi Track
Header Menu Copy
Header Menu Copy Time
Header Menu Create Follow Action Chain
Header Menu Cross Fade
Header Menu Cut
Header Menu Cut Time
Header Menu Delete
Header Menu Delete All Take Lanes
Header Menu Delete Cue
Header Menu Delete Env
Header Menu Delete Time
Header Menu Delete Time Signature Change
Header Menu Delete Unused Take Lanes
Header Menu Disable Notes
Header Menu Disable Selection
Header Menu Disable Tracks
Header Menu Draw Mode
Header Menu Dry Wet
Header Menu Duplicate
Header Menu Duplicate Loop
Header Menu Duplicate Time
Header Menu Duplicate Unlinked Loop
Header Menu Edit Info Text
Header Menu Edit Key Map
Header Menu Edit Midi Map
Header Menu Edit Quick Tags
Header Menu Enable Browser Pre Hear
Header Menu Enable Midi Keyboard
Header Menu Enable Midi Prelisten
Header Menu Expand Clip Detail View
Header Menu Export Midi Clip
Header Menu External Sync
Header Menu Extract Groove
Header Menu File Manager
Header Menu Find
Header Menu Focus Next
Header Menu Focus Next Neighbor
Header Menu Focus Previous
Header Menu Focus Previous Neighbor
Header Menu Follow
Header Menu Freeze
Header Menu Full Height Browser
Header Menu Fullscreen
Header Menu Fx Slots
Header Menu Grid Fixed
Header Menu Grid Narrow
Header Menu Grid Quintuplet
Header Menu Grid Septuplet
Header Menu Grid Triplet
Header Menu Grid Wide
Header Menu Group Notes
Header Menu Group Selection
Header Menu Group Tracks
Header Menu Help About
Header Menu Help Authorizations And Trial
Header Menu Help Check For Updates
Header Menu Help Community
Header Menu Help Home Page
Header Menu Help Manual
Header Menu Help Purchase License
Header Menu Help Support
Header Menu Hide All Extra Columns
Header Menu Import File
Header Menu In Out
Header Menu Info
Header Menu Insert Captured Scene
Header Menu Insert Cue
Header Menu Insert Fade
Header Menu Insert Midi Clip
Header Menu Insert Midi Track
Header Menu Insert Scene
Header Menu Insert Send
Header Menu Insert Silence
Header Menu Insert Special
Header Menu Insert Take Lane
Header Menu Insert Time
Header Menu Insert Time Signature Change
Header Menu Insert Track
Header Menu Install Package
Header Menu Invert Selection
Header Menu Join Notes
Header Menu Latency Compensation
Header Menu Latency Compensation Play Through
Header Menu Launch Quantisation Enable
Header Menu Launch Quantisation Narrower
Header Menu Launch Quantisation Triplet
Header Menu Launch Quantisation Wider
Header Menu Legacy Hi Q Mode
Header Menu Lessons
Header Menu Link Start Stop Sync
Header Menu Load Demo Set
Header Menu Lock Envelope
Header Menu Loop Selection
Header Menu Manage Projects
Header Menu Max For Live Developer Mode
Header Menu Maxfor Live Window
Header Menu Metronome While Recording
Header Menu Mixer
Header Menu Mono Side Chain
Header Menu Move Insert Marker To Playhead Position
Header Menu New
Header Menu Open
Header Menu Over View
Header Menu Pack Overview
Header Menu Paste
Header Menu Paste Bounced Audio
Header Menu Paste Clips
Header Menu Paste Time
Header Menu Performance Impact
Header Menu Pfl Switches
Header Menu Play
Header Menu Play Arrangement Selection
Header Menu Plugin Windows
Header Menu Pref
Header Menu Q Eight
Header Menu Q Eight And Eight Triplets
Header Menu Q Eight Triplets
Header Menu Q No
Header Menu Q Quarter
Header Menu Q Sixteenth
Header Menu Q Sixteenth And Sixteenth Triplets
Header Menu Q Sixteenth Triplets
Header Menu Q Thirtysecond
Header Menu Quantize
Header Menu Quantize Settings
Header Menu Quit
Header Menu Record In Arrangement
Header Menu Redo
Header Menu Rename
Header Menu Render To Disk
Header Menu Render To Disk No Video
Header Menu Reset Nonautomated Midi Controllers On Clip Starts
Header Menu Retune Set On Tuning System Change
Header Menu Return Play Position
Header Menu Returns
Header Menu Ruler Normal
Header Menu Ruler Smpte24
Header Menu Ruler Smpte25
Header Menu Ruler Smpte29
Header Menu Ruler Smpte30 Drop
Header Menu Ruler Smpte30 Non Drop
Header Menu Save
Header Menu Save A Copy
Header Menu Save As
Header Menu Save As Default Set
Header Menu Save As Template
Header Menu Save Self Contained
Header Menu Scale Highlighting
Header Menu Scene Tempo And Time Signature
Header Menu Second Window
Header Menu Select All
Header Menu Select Loop
Header Menu Sends
Header Menu Separate Stems To New Audio Tracks
Header Menu Set Clip End
Header Menu Set Clip Start
Header Menu Set Loop End
Header Menu Set Loop Start
Header Menu Show All Tracks
Header Menu Show Browser
Header Menu Show Clip View
Header Menu Show Device View
Header Menu Show Extra Column
Header Menu Show File Extensions
Header Menu Show Groove Pool
Header Menu Show Similar Files
Header Menu Show Tuning Pool
Header Menu Slice To New Midi Track
Header Menu Snap Automation To Grid
Header Menu Snap To Grid
Header Menu Solo In Place
Header Menu Solo Switches
Header Menu Span
Header Menu Split
Header Menu Split Clip From Within
Header Menu Split Notes
Header Menu Start Transport With Record
Header Menu Start Transport With Tap Tempo
Header Menu Step Input Mode
Header Menu Switch To Next Mode
Header Menu Switch To Previous Mode
Header Menu Text Clip Stop
Header Menu Toggle Arrangement Session
Header Menu Toggle Clip Or Device View
Header Menu Toggle Detail
Header Menu Toggle Metronome
Header Menu Track Arming
Header Menu Track Options
Header Menu Track Solo
Header Menu Un Group Selection
Header Menu Undo
Header Menu Undo History Browser
Header Menu Ungroup Notes
Header Menu Ungroup Tracks
Header Menu Use Tab To Navigate Controls
Header Menu Video Window
Header Menu Volume
Header Menu Wrap Keyboard Navigation
Header Menu Zoom Back Out Time
Header Menu Zoom In Time
Header Menu Zoom In Window
Header Menu Zoom Out Time
Header Menu Zoom Out Window
Header Menu Zoom To Selection
Header Menu Zoom Window To Default
Header Meter
Header Mid Freq
Header Midi
Header Midi Generative Tools
Header Midi Note Number Spelling
Header Midi Stretch Marker
Header Midi To
Header Midi Transformation Tools
Header Min Refresh Time
Header Mixer
Header Mod Channel Pressure
Header Mod Foot
Header Mod Key
Header Mod Pitch Bend
Header Mod Pitch By Mpe Pitch Bend
Header Mod Pitch By Pitch Bend
Header Mod Rel Vel
Header Mod Vel
Header Mod Wheel
Header Mode
Header Monitor
Header Mono Stereo Inputs Outputs Control
Header Move Pack
Header Mpe Enabled
Header Mpe Mode
Header Mpe Pitch Bend
Header Mpe Settings
Header Mpe Slide
Header Mtc Frame Rate
Header Mtc Start Offset
Header Multi Clip Loop Bars
Header Multi Clip Titles
Header Mute Button
Header Name
Header Next Page
Header Next Similar
Header No Clip
Header No Triplets
Header Noise Width
Header Normalize Panorama
Header Normalize Volume
Header Note Editor View Mode
Header Note Expression Editor
Header Note Tools
Header Num Averages
Header Num Harmonics
Header Num Voices
Header Offset
Header On
Header Open Info
Header Operation
Header Operator Display
Header Operator Finger
Header Operator Shell
Header Osc Feedback
Header Osc Mode
Header Osc On Off
Header Osc Phase
Header Osc Preview
Header Osc Repeat
Header Osc Retrigger
Header Osc Tune Coarse
Header Osc Tune Fine
Header Osc Tune Fixed
Header Osc Tune Fixed Multi
Header Osc Tune Is Fixed
Header Osc Type
Header Osc Volume
Header Osc Volume By Vel
Header Osc Wave Form
Header Osci Coarse
Header Osci Coarse Vel
Header Osci Coarse Vel Quant
Header Osci Fine
Header Osci Fixed Multi
Header Osci Fixed On
Header Osci Fixed Value
Header Osci Key Scale
Header Osci On
Header Osci Vel Scale
Header Osci Volume
Header Oscillator Section
Header Out
Header Out Latency
Header Output
Header Output Buffer Size
Header Output Device
Header Output Device Remote
Header Output Device Sync
Header Output Device Track
Header Output Gain
Header Output Meter
Header Output Mode
Header Over View
Header Overall Latency
Header Pack Overview
Header Pan
Header Pan Key
Header Pan Random
Header Pane Filter And Globals
Header Pane MIDI
Header Pane Modulations
Header Pane Pitch And Osc
Header Pane Sample
Header Pane Zone
Header Panorama
Header Panorama By Rnd
Header Parameter Chooser
Header Parameter Region High
Header Parameter Region Low
Header Path
Header Pause Analysis
Header Peak Reset Button
Header Phase
Header Phase Nudge Down
Header Phase Nudge Up
Header Pitch
Header Pitch Bend Range
Header Pitch Class
Header Pitch Coarse
Header Pitch Decay
Header Pitch Env
Header Pitch Env Amount
Header Pitch Env On Off
Header Pitch Envelope Amount
Header Pitch Envelope Amount A
Header Pitch Envelope Amount B
Header Pitch Envelope Connection B
Header Pitch Envelope On
Header Pitch Envelope To Lfo
Header Pitch Envelope To Osc
Header Pitch Fine
Header Pitch Finger
Header Play By Key
Header Play Start Mark
Header Portamento Mode
Header Portamento Time
Header Pre Dc Filter
Header Pre Hear
Header Pre Post
Header Preferences Page Chooser
Header Prelisten In
Header Previous Lesson
Header Previous Page
Header Previous Similar
Header Project
Header Pseudo Warp Marker
Header Random
Header Range
Header Ranges Editor
Header Rate Beats
Header Rate Frequency
Header Rate Type
Header Re Wire Device Sample Rate
Header Record Monitoring Latency
Header Release
Header Release Notes
Header Reload Loops From File
Header Remote Script Dump
Header Remote Script Entry
Header Remote Script In Device
Header Remote Script Out Device
Header Remove From Sidebar
Header Remove Lane
Header Rename Keyword
Header Repeat
Header Reset Filter Groups To Default
Header Reset Slices
Header Reset Tag Order
Header Reset To Default Icon
Header Resume Analysis
Header Resync Ext Hardware Button Show
Header Retrigger
Header Return Slots
Header Return To Reference
Header Reveal In Finder
Header Reverse Sample
Header Review Media Files
Header Root Note
Header Root Note Spelling
Header Round Robin
Header Round Robin Mode
Header Round Robin Reset Interval
Header Routing Target
Header Ruler Piano
Header Ruler Zoom Scroll
Header Sample Analyse
Header Sample Channel Selector
Header Sample Detune
Header Sample Editor View Mode
Header Sample Interpolation Mode
Header Sample Link
Header Sample Panorama
Header Sample Ram
Header Sample Rate
Header Sample Release Loop Crossfade In Samples
Header Sample Release Loop Crossfade In Seconds
Header Sample Release Loop Detune
Header Sample Release Loop Mode
Header Sample Release Loop Start In Samples
Header Sample Release Loop Start In Seconds
Header Sample Reverse
Header Sample Rootkey
Header Sample Sample
Header Sample Sample End In Samples
Header Sample Sample End In Seconds
Header Sample Sample Start In Samples
Header Sample Sample Start In Seconds
Header Sample Selected Sample
Header Sample Snap
Header Sample Sustain Loop Crossfade In Samples
Header Sample Sustain Loop Crossfade In Seconds
Header Sample Sustain Loop Detune
Header Sample Sustain Loop End In Samples
Header Sample Sustain Loop End In Seconds
Header Sample Sustain Loop Mode
Header Sample Sustain Loop Start In Samples
Header Sample Sustain Loop Start In Seconds
Header Sample Tune Scale
Header Sample Vertical Zoom
Header Sample Volume
Header Save
Header Save As Default Configuration
Header Save Groove
Header Save Pad Default
Header Scale Name
Header Scale Non Dpi Aware Plugin
Header Scale X
Header Scale Y Max
Header Scale Y Min
Header Scene Number
Header Scene Tempo
Header Scrub Area
Header Search Folder Path
Header Search In Custom Folder
Header Search In Library
Header Search In Project
Header Select All With Same Range
Header Selected Device
Header Selection
Header Selection Mark
Header Selector Range Button
Header Selector Range Ruler
Header Send
Header Send Panel
Header Session
Header Session Record
Header Session Recording
Header Set Default Template
Header Set Pitch Range
Header Set Search Folder
Header Set Song Start Time
Header Shaper Amount
Header Shaper Display
Header Shaper Drive
Header Shaper Dry Wet
Header Shaper On Off
Header Shaper Type
Header Show All Filter Groups
Header Show Bins
Header Show Branch Names In Editor
Header Show Browser Config Menu
Header Show Candidates
Header Show Clips
Header Show Defects
Header Show External Elsewhere Media Files
Header Show External Library Media Files
Header Show External Project Media Files
Header Show Hide Browser
Header Show Hide Filter
Header Show Hide Tag Editor
Header Show In Browser
Header Show In Explorer
Header Show In Folder
Header Show In Places
Header Show Launch Panel
Header Show Max
Header Show Media Files
Header Show Mixer Config Menu
Header Show Note Values For Filter Frequency
Header Show Old Version
Header Show Options
Header Show Other Unused Media Files
Header Show Pack Info
Header Show Presets
Header Show Quick Tags
Header Show ROM Media Files
Header Show Sets
Header Show Similar Files
Header Show Tag Editor
Header Show Take Lanes
Header Show Unused Bounce Samples
Header Show Unused Consolidate Samples
Header Show Unused Freeze Samples
Header Show Unused Recording Samples
Header Sidebar Add Folder
Header Sidebar All
Header Sidebar Audio Effects
Header Sidebar Clips
Header Sidebar Cloud
Header Sidebar Colors
Header Sidebar Current Project
Header Sidebar Drums
Header Sidebar Grooves
Header Sidebar Group Browse
Header Sidebar Group Collections
Header Sidebar Group Places
Header Sidebar Local Transfer
Header Sidebar M4 L
Header Sidebar Midi Effects
Header Sidebar Modulators
Header Sidebar Packs
Header Sidebar Plugins
Header Sidebar Samples
Header Sidebar Saved Views
Header Sidebar Sounds
Header Sidebar Splice
Header Sidebar Synths
Header Sidebar Temp
Header Sidebar Templates
Header Sidebar Tunings
Header Sidebar Unusable File System Location
Header Sidebar User Folders
Header Sidebar User Library
Header Signature
Header Signature Denominator
Header Signature Num
Header Simpler Modulated Loop
Header Simpler Multisample Mode
Header Single Track Back To Arrangement
Header Slice To Drum Rack
Header Slice To New Midi Track
Header Slicing Dialog
Header Solo
Header Solo Button
Header Solo Cue
Header Solo Pfl Switch
Header Song Start Mark
Header Sort Parts
Header Sound Analysis Status View
Header Source Highlight
Header Speaker
Header Spread
Header Status
Header Step Scene Down
Header Step Scene Up
Header Stepped Discrete Modulation
Header Stereo Split Pan L
Header Stereo Split Pan R
Header Stop
Header Stop Hot Swap
Header Stop Main
Header Structure
Header Swap Next Similar Sample
Header Swap Previous Similar Sample
Header Sync In Latency
Header Sync Input General
Header Sync Input Type
Header Sync Midi Clock Type
Header Sync Out Latency
Header Sync Output General
Header Sync Resync Style
Header Take Lane
Header Take Lane Content
Header Take Over
Header Tempo Coarse
Header Tempo Fine
Header Tempo Follower
Header Test Cpu Usage
Header Test Freq
Header Test General
Header Test Tone
Header Test Volume
Header Time
Header Time By Key
Header Time Key
Header Time Range Selection Bar
Header Time Ruler
Header Time Signature Marker
Header Time Signature Markers
Header Title Bar
Header Toggle Automation
Header Toggle Clip View Button
Header Toggle Device View Button
Header Toggle Mixer View Button
Header Tone
Header Tool Panels
Header Track Bypass Tuning
Header Track Delay Compensation
Header Track Header
Header Track Return Header
Header Track Tuning Midi Mapper
Header Track Tuning Midi Mapping Config Button
Header Transform Handle Corner
Header Transform Handle Left Right
Header Transform Handle Top Bottom
Header Transients
Header Transmission
Header Transpose
Header Transpose Cents
Header Transpose Semitones
Header Tune Pitch Bend
Header Undo History
Header Unfold
Header Unfold Track
Header Unfollow Tempo Automation
Header Uninstall Live Pack
Header Update Similar Sample Reference
Header Upgrade From Legacy Mode
Header Upper
Header Used By Current Set
Header User Color00
Header User Color01
Header User Color02
Header User Color03
Header User Color04
Header User Color05
Header User Color06
Header User Color07
Header User Color08
Header User Color09
Header User Color10
Header User Color11
Header User Color12
Header User Color13
Header User Color14
Header User Color15
Header User Color16
Header User Color17
Header User Color18
Header User Color19
Header User Color20
Header User Color21
Header User Color22
Header User Color23
Header User Color24
Header User Color25
Header User Color26
Header User Color27
Header User Color28
Header User Color29
Header User Color30
Header User Color31
Header User Color32
Header User Color33
Header User Color34
Header User Color35
Header User Color36
Header User Color37
Header User Color38
Header User Color39
Header User Color40
Header User Color41
Header User Color42
Header User Color43
Header User Color44
Header User Color45
Header User Color46
Header User Color47
Header User Color48
Header User Color49
Header User Color50
Header User Color51
Header User Color52
Header User Color53
Header User Color54
Header User Color55
Header User Color56
Header User Color57
Header User Color58
Header User Color59
Header User Color60
Header User Color61
Header User Color62
Header User Color63
Header User Color64
Header User Color65
Header User Color66
Header User Color67
Header User Color68
Header User Color69
Header User Prefix
Header Variation
Header Variation Mode
Header Velocity
Header Velocity Range Button
Header Velocity Range Ruler
Header Voices
Header Volume
Header Volume By Vel
Header Volume Decay
Header Volume Section
Header Warp Marker
Header Warp Mode
Header Waveform
Header With Clip
Header Zone Layer Crossfade
Header Chain Volume
Header Left Handle
Header Right Handle
Header Toggle Button
Hi Quality
Hi Quality Oversampling
Hide All Filter Groups
Hide Filter
Hide From Sidebar
Hide Scale Controls
Histogram Gr Mode
Histogram Out Mode
Home File Manager
Home Lessons
Home Pack Overview
Horizontal Split
Hot Swap
Hot Swap Button
Hot Swap Clip Groove
Hot Swap Groove
In
In Key
In Key Audio Track
In Key Drum Track
In Latency
In Out Panel
Include Elsewhere Media Files
Include External Elsewhere Media Files
Include External Library Media Files
Include External Project Media Files
Include Library Media Files
Include Project Media Files
Include ROM Media Files
Input
Input Buffer Size
Input Device
Input Device Remote
Input Device Sync
Input Device Track
Input Gain
Input Meter
Inputs
Insert Mark
Inspect Current Document
Inspect Current Project
Inspect Library
Interpolation
Interval
Invalid Tuning File
Invert Phase
Is Active
Is Current Set
Is In Current Set
Jack Buffer Size
Jump Next Cue
Jump Prev Cue
Key Range Button
Key Range Ruler
Key Zone Shift
Keyboard Conflict
LFO Section
Latency
Leave Hot Swap
Led
Legacy Smoothing
Length
Lessons
Lfo Amount
Lfo Amount A
Lfo Amount B
Lfo Amount Vel
Lfo Attack
Lfo Connection B
Lfo Display
Lfo Finger
Lfo Offset
Lfo On
Lfo On Off
Lfo One Filter
Lfo One Panorama
Lfo One Pitch
Lfo One Volume
Lfo Phase
Lfo Range
Lfo Rate
Lfo Rate Beats
Lfo Rate By Key
Lfo Rate Freq
Lfo Rate Key
Lfo Rate Type
Lfo Retrigger
Lfo Spin
Lfo Stereo Mode
Lfo To Filter
Lfo To Osc
Lfo Type
Lfo Waveform
Lfo Width
Link
Link Audio
Link Audio Latency
Link Audio Peer Name
Link Audio Sync To Incoming Audio
Linked Track Indicator
List
Locate Media Files
Locate Patch File
Location
Lock
Loop Bar Resize
Loop End
Loop Start
Lower
MPE Editor View Mode
Macro Control
Macro Control Map Button
Macro Control Name
Macro Range Button
Macro Range Ruler
Main Automation Lane
Main Clip Properties
Main Out
Main Pan
Main Range
Main Stereo Split Pan
Main Track Header
Main Volume
Manage Sample
Maneuver Link To Audio
Mapping Browser
Menu Accessibility Speak Info Text
Menu Add Cue
Menu Add Folder To Browser
Menu Add Time Signature Change
Menu Apply Algorithm
Menu Arm Record In Arrangement
Menu Arrangement In Out
Menu Arrangement Overdub
Menu Arrangement Returns
Menu Arrangement Track Options
Menu Arrangement Volume
Menu Automation Mode
Menu Bounce To New Track
Menu Bounce Track In Place
Menu Browser History Back
Menu Browser History Forward
Menu Capture Midi
Menu Chase Notes
Menu Clean Recent
Menu Clear All Colors
Menu Clear Recent
Menu Clip Detail View Layout Auto
Menu Clip Detail View Layout Horizontally
Menu Clip Detail View Layout Vertically
Menu Close
Menu Consolidate
Menu Consolidate To New Scene
Menu Continue Playback
Menu Convert Drums To New Midi Track
Menu Convert Harmony To New Midi Track
Menu Convert Melody To New Midi Track
Menu Copy
Menu Copy Time
Menu Create Follow Action Chain
Menu Cross Fade
Menu Cut
Menu Cut Time
Menu Delete
Menu Delete All Take Lanes
Menu Delete Cue
Menu Delete Env
Menu Delete Time
Menu Delete Time Signature Change
Menu Delete Unused Take Lanes
Menu Disable Notes
Menu Disable Selection
Menu Disable Tracks
Menu Disclosure Menu Button
Menu Draw Mode
Menu Dry Wet
Menu Duplicate
Menu Duplicate Loop
Menu Duplicate Time
Menu Duplicate Time Unlinked Envelope
Menu Duplicate Unlinked Loop
Menu Edit Info Text
Menu Edit Key Map
Menu Edit Midi Map
Menu Edit Quick Tags
Menu Enable Browser Pre Hear
Menu Enable Midi Keyboard
Menu Enable Midi Prelisten
Menu Expand Clip Detail View
Menu Export Midi Clip
Menu External Sync
Menu Extract Groove
Menu File Manager
Menu Find
Menu Focus Next
Menu Focus Next Neighbor
Menu Focus Previous
Menu Focus Previous Neighbor
Menu Follow
Menu Freeze
Menu Full Height Browser
Menu Fullscreen
Menu Fx Slots
Menu Grid Fixed
Menu Grid Narrow
Menu Grid Quintuplet
Menu Grid Septuplet
Menu Grid Triplet
Menu Grid Wide
Menu Group Notes
Menu Group Selection
Menu Group Tracks
Menu Help About
Menu Help Authorizations And Trial
Menu Help Check For Updates
Menu Help Community
Menu Help Home Page
Menu Help Manual
Menu Help Purchase License
Menu Help Support
Menu Hide All Extra Columns
Menu Import File
Menu In Out
Menu Info
Menu Insert Captured Scene
Menu Insert Cue
Menu Insert Fade
Menu Insert Midi Clip
Menu Insert Midi Track
Menu Insert Scene
Menu Insert Send
Menu Insert Silence
Menu Insert Special
Menu Insert Take Lane
Menu Insert Time
Menu Insert Time Signature Change
Menu Insert Track
Menu Install Package
Menu Invert Selection
Menu Join Notes
Menu Latency Compensation
Menu Latency Compensation Play Through
Menu Launch Quantisation Enable
Menu Launch Quantisation Narrower
Menu Launch Quantisation Triplet
Menu Launch Quantisation Wider
Menu Legacy Hi Q Mode
Menu Link Start Stop Sync
Menu Load Demo Set
Menu Lock Envelope
Menu Loop Selection
Menu Manage Projects
Menu Max For Live Developer Mode
Menu Max For Live Window
Menu Metronome While Recording
Menu Mixer
Menu Mono Side Chain
Menu Move Insert Marker To Playhead Position
Menu New
Menu Open
Menu Over View
Menu Pack Overview
Menu Paste
Menu Paste Bounced Audio
Menu Paste Clips
Menu Paste Time
Menu Performance Impact
Menu Pfl Switches
Menu Play
Menu Play Arrangement Selection
Menu Plugin Windows
Menu Pref
Menu Q Eight
Menu Q Eight And Eight Triplets
Menu Q Eight Triplets
Menu Q No
Menu Q Quarter
Menu Q Sixteenth
Menu Q Sixteenth And Sixteenth Triplets
Menu Q Sixteenth Triplets
Menu Q Thirtysecond
Menu Quantize
Menu Quantize Settings
Menu Quit
Menu Recent Unavailable
Menu Record In Arrangement
Menu Redo
Menu Rename
Menu Render To Disk
Menu Render To Disk No Video
Menu Reset Nonautomated Midi Controllers On Clip Starts
Menu Retune Set On Tuning System Change
Menu Return Play Position
Menu Returns
Menu Ruler Normal
Menu Ruler Smpte24
Menu Ruler Smpte25
Menu Ruler Smpte29
Menu Ruler Smpte30 Drop
Menu Ruler Smpte30 Non Drop
Menu Save
Menu Save A Copy
Menu Save As
Menu Save As Default Set
Menu Save As Template
Menu Save Self Contained
Menu Scale Highlighting
Menu Scene Tempo And Time Signature
Menu Second Window
Menu Select All
Menu Select Loop
Menu Sends
Menu Separate Stems To New Audio Tracks
Menu Set Clip End
Menu Set Clip Start
Menu Set Loop End
Menu Set Loop Start
Menu Show Browser
Menu Show Clip View
Menu Show Device View
Menu Show Extra Column
Menu Show File Extensions
Menu Show Groove Pool
Menu Show Similar Files
Menu Show Tuning Pool
Menu Slice To New Midi Track
Menu Snap Automation To Grid
Menu Snap To Grid
Menu Solo In Place
Menu Solo Switches
Menu Span
Menu Split
Menu Split Clip From Within
Menu Split Notes
Menu Start Transport With Record
Menu Start Transport With Tap Tempo
Menu Switch To Next Mode
Menu Switch To Previous Mode
Menu Text Clip Stop
Menu Toggle Arrangement Session
Menu Toggle Clip Or Device View
Menu Toggle Detail
Menu Toggle Metronome
Menu Track Arming
Menu Track Options
Menu Track Solo
Menu Un Group Selection
Menu Undo
Menu Undo History Browser
Menu Ungroup Notes
Menu Ungroup Tracks
Menu Use Tab To Navigate Controls
Menu Video Window
Menu Volume
Menu Wrap Keyboard Navigation
Menu Zoom Back Out Time
Menu Zoom In Time
Menu Zoom In Window
Menu Zoom Out Time
Menu Zoom Out Window
Menu Zoom To Selection
Menu Zoom Window To Default
Meter
Mid Freq
Midi
Midi Content
Midi Generative Tools
Midi Note Number Spelling
Midi Stretch Marker
Midi To
Midi Track
Midi Transformation Tools
Min Refresh Time
Mixer
Mod Channel Pressure
Mod Channel Pressure Text
Mod Foot
Mod Foot Text
Mod Key
Mod Key Text
Mod Pitch Bend
Mod Pitch Bend Text
Mod Pitch By Mpe Pitch Bend
Mod Pitch By Pitch Bend
Mod Rel Vel
Mod Rel Vel Text
Mod Vel
Mod Vel Text
Mod Wheel
Mod Wheel Text
Mode
Monitor
Mono
Mono Stereo Inputs Outputs Control
Move Pack
Mpe Enabled
Mpe Mode
Mpe Pitch Bend
Mpe Pitch Bend Text
Mpe Settings
Mpe Slide
Mpe Slide Text
Mtc Frame Rate
Mtc Start Offset
Multi Clip Loop Bars
Multi Clip Titles
Mute Button
Name
Next Page
Next Similar
No Clip
No Triplets
Noise Width
Normalize Panorama
Normalize Volume
Not Over Note
Note Editor View Mode
Note Expression Editor
Note Tools
Num Averages
Num Harmonics
Num Voices
Offset
On
Open Info
Operation
Operator Display
Operator Finger
Operator Shell
Option Mac
Option Pc
Osc Basic Shapes
Osc Basic Shapes M1
Osc Basic Shapes M2
Osc Bit Grunge
Osc Bit Grunge M1
Osc Bit Grunge M2
Osc Bubble
Osc Bubble M1
Osc Bubble M2
Osc Chip
Osc Chip M1
Osc Chip M2
Osc Chord
Osc Chord M1
Osc Chord M2
Osc Crackle
Osc Crackle M1
Osc Crackle M2
Osc Dual Basic Shapes
Osc Dual Basic Shapes M1
Osc Dual Basic Shapes M2
Osc Extratone
Osc Extratone M1
Osc Extratone M2
Osc Feedback
Osc Filtered Noise
Osc Filtered Noise M1
Osc Filtered Noise M2
Osc Fold Fm
Osc Fold Fm M1
Osc Fold Fm M2
Osc Harmonic Fm
Osc Harmonic Fm M1
Osc Harmonic Fm M2
Osc Mode
Osc Noise Loop
Osc Noise Loop M1
Osc Noise Loop M2
Osc Noisy Shapes
Osc Noisy Shapes M1
Osc Noisy Shapes M2
Osc Nonexistent
Osc On Off
Osc Phase
Osc Preview
Osc Rain
Osc Rain M1
Osc Rain M2
Osc Repeat
Osc Retrigger
Osc Shepards Pi
Osc Shepards Pi M1
Osc Shepards Pi M2
Osc Simple Fm
Osc Simple Fm M1
Osc Simple Fm M2
Osc Square5th
Osc Square5th M1
Osc Square5th M2
Osc Square Sync
Osc Square Sync M1
Osc Square Sync M2
Osc Squelch
Osc Squelch M1
Osc Squelch M2
Osc Sub
Osc Sub M1
Osc Sub M2
Osc Swarm Saw
Osc Swarm Saw M1
Osc Swarm Saw M2
Osc Swarm Sine
Osc Swarm Sine M1
Osc Swarm Sine M2
Osc Swarm Square
Osc Swarm Square M1
Osc Swarm Square M2
Osc Swarm Triangle
Osc Swarm Triangle M1
Osc Swarm Triangle M2
Osc Tarp
Osc Tarp M1
Osc Tarp M2
Osc Tune Coarse
Osc Tune Fine
Osc Tune Fixed
Osc Tune Fixed Multi
Osc Tune Is Fixed
Osc Type
Osc Volume
Osc Volume By Vel
Osc Wave Form
Osci Coarse
Osci Coarse Vel
Osci Coarse Vel Quant
Osci Fine
Osci Fixed Multi
Osci Fixed On
Osci Fixed Value
Osci Key Scale
Osci On
Osci Vel Scale
Osci Volume
Oscillator Section
Out
Out Latency
Output
Output Buffer Size
Output Device
Output Device Remote
Output Device Sync
Output Device Track
Output Gain
Output Meter
Output Mode
Outputs
Over Note
Over View
Overall Latency
Pack Overview
Pan
Pan Key
Pan Random
Pane Filter And Globals
Pane MIDI
Pane Modulations
Pane Pitch And Osc
Pane Sample
Pane Zone
Panorama
Panorama By Rnd
Parameter Chooser
Parameter Region High
Parameter Region Low
Path
Pause Analysis
Peak Reset Button
Phase
Phase Nudge Down
Phase Nudge Up
Pitch
Pitch Bend Range
Pitch Class
Pitch Coarse
Pitch Decay
Pitch Env
Pitch Env Amount
Pitch Env On Off
Pitch Envelope Amount
Pitch Envelope Amount A
Pitch Envelope Amount B
Pitch Envelope Connection B
Pitch Envelope On
Pitch Envelope To Lfo
Pitch Envelope To Osc
Pitch Fine
Pitch Finger
Play By Key
Play Start Mark
Portamento Mode
Portamento Time
Pre Dc Filter
Pre Hear
Pre Post
Preferences Page Chooser
Prelisten
Previous Lesson
Previous Page
Previous Similar
Project
Pseudo Warp Marker
Random
Range
Ranges Editor
Ranges Editor Toggle Button
Rate Beats
Rate Frequency
Rate Type
Re Wire Device Sample Rate
Record Monitoring Latency
Release
Release Notes
Reload Loops From File
Remote Script Dump
Remote Script Entry
Remote Script In Device
Remote Script Out Device
Remove From Sidebar
Remove Lane
Rename Keyword
Render Real Time
Render Sample Rate
Render Sample Rate When Downsampling
Repeat
Reset Filter Groups To Default
Reset Slices
Reset Tag Order
Reset To Default Icon
Resume Analysis
Resync Ext Hardware Button Show
Retrigger
Return Slots
Return To Reference
Reveal In Finder
Reverse Sample
Review Media Files
Root Note
Root Note Audio Track
Root Note Drum Track
Root Note Spelling
Round Robin
Round Robin Mode
Round Robin Reset Interval
Routing Target
Ruler Piano
Ruler Zoom Scroll
Sample Analyse
Sample Channel Selector
Sample Detune
Sample Editor View Mode
Sample Interpolation Mode
Sample Link
Sample Panorama
Sample Ram
Sample Rate
Sample Release Loop Crossfade In Samples
Sample Release Loop Crossfade In Seconds
Sample Release Loop Detune
Sample Release Loop Mode
Sample Release Loop Start In Samples
Sample Release Loop Start In Seconds
Sample Reverse
Sample Rootkey
Sample Sample
Sample Sample End In Samples
Sample Sample End In Seconds
Sample Sample Start In Samples
Sample Sample Start In Seconds
Sample Selected Sample
Sample Snap
Sample Sustain Loop Crossfade In Samples
Sample Sustain Loop Crossfade In Seconds
Sample Sustain Loop Detune
Sample Sustain Loop End In Samples
Sample Sustain Loop End In Seconds
Sample Sustain Loop Mode
Sample Sustain Loop Start In Samples
Sample Sustain Loop Start In Seconds
Sample Tune Scale
Sample Vertical Zoom
Sample Volume
Save
Save As Default Configuration
Save Groove
Save Pad Default
Scale Name
Scale Name Audio Track
Scale Name Drum Track
Scale Non Dpi Aware Plugin
Scale X
Scale Y Max
Scale Y Min
Scene Follow Action
Scene Follow Action Enabled
Scene Follow Action Jump Index
Scene Number
Scene Tempo
Scrub Area
Search Folder Path
Search In Custom Folder
Search In Library
Search In Project
Select All With Same Range
Selected Device
Selection
Selection Mac
Selection Mark
Selection Pc
Selector Range Button
Selector Range Ruler
Send
Send Panel
Session
Session Record
Session Recording
Set Default Template
Set Pitch Range
Set Search Folder
Set Song Start Time
Shaper Amount
Shaper Display
Shaper Drive
Shaper Dry Wet
Shaper On Off
Shaper Type
Shift Mac
Shift Pc
Show All Filter Groups
Show Bins
Show Branch Names In Editor
Show Browser Config Menu
Show Candidates
Show Clips
Show Defects
Show External Elsewhere Media Files
Show External Library Media Files
Show External Project Media Files
Show Hide Browser
Show Hide Filter
Show Hide Tag Editor
Show In Browser
Show In Explorer
Show In Folder
Show In Places
Show Launch Panel
Show Max
Show Media Files
Show Mixer Config Menu
Show Note Values For Filter Frequency
Show Old Version
Show Options
Show Other Unused Media Files
Show Pack Info
Show Presets
Show Quick Tags
Show ROM Media Files
Show Sets
Show Similar Files
Show Tag Editor
Show Take Lanes
Show Unused Bounce Samples
Show Unused Consolidate Samples
Show Unused Freeze Samples
Show Unused Recording Samples
Sidebar Add Folder
Sidebar All
Sidebar Audio Effects
Sidebar Clips
Sidebar Cloud
Sidebar Colors
Sidebar Current Project
Sidebar Drums
Sidebar Grooves
Sidebar Group Browse
Sidebar Group Collections
Sidebar Group Places
Sidebar Local Transfer
Sidebar M4 L
Sidebar Midi Effects
Sidebar Modulators
Sidebar Packs
Sidebar Plugins
Sidebar Samples
Sidebar Saved View
Sidebar Sounds
Sidebar Splice
Sidebar Synths
Sidebar Temp
Sidebar Templates
Sidebar Tunings
Sidebar Unusable File System Location
Sidebar User Folders
Sidebar User Library
Signature
Signature Denominator
Signature Num
Simpler Modulated Loop
Simpler Multisample Mode
Single Track Back To Arrangement
Slice To Drum Rack
Slice To New Midi Track
Slicing Dialog
Slide Clip Content Mac
Slide Clip Content Pc
Solo
Solo Button
Solo Cue
Solo Pfl Switch
Song Start Mark
Sort Parts
Sound Analysis Status View
Source Highlight
Speaker
Special Mac
Special Pc
Spread
Start Stop Sync
Status
Step Input Mode
Step Scene Down
Step Scene Up
Stepped Discrete Modulation
Stereo
Stereo Split Pan
Stop
Stop Hot Swap
Stop Main
Structure
Swap Next Similar Sample
Swap Previous Similar Sample
Sync In Latency
Sync Input General
Sync Input Type
Sync Midi Clock Type
Sync Out Latency
Sync Output General
Sync Resync Style
Tag Editor View
Take Lane
Take Lane Content
Take Over
Tempo Coarse
Tempo Fine
Tempo Follower
Test Cpu Usage
Test Freq
Test General
Test Tone
Test Volume
Time
Time By Key
Time Key
Time Range Selection Bar
Time Ruler
Time Selection Mac
Time Selection Pc
Time Signature Marker
Title Bar
Toggle Automation
Toggle Clip View Button
Toggle Device View Button
Toggle Mixer View Button
Tone
Tool Panels
Track Bypass Tuning
Track Delay Compensation
Track Header
Track Return Header
Track Tuning Midi Mapper
Track Tuning Midi Mapping Config Button
Transform Handle Corner
Transform Handle Left Right
Transform Handle Top Bottom
Transients
Transmission
Transpose
Transpose Cents
Transpose Semitones
Tune Pitch Bend
Undo History
Unfold
Unfold Track
Unfollow Tempo Automation
Uninstall Live Pack
Unsaved Changes
Unsupported Similarity Swap Suffix
Update Similar Sample Reference
Upgrade From Legacy Mode
Upper
Used By Current Set
Variation
Variation Mode
Vcredist Needs Installation
Velocity
Velocity Range Button
Velocity Range Ruler
View Pan Mac
View Pan Pc
Voices
Volume
Volume By Vel
Volume Decay
Volume Section
Warp Marker
Warp Mode
Warp Mode Complex
Waveform
With Clip
With Parameter
Without Parameter
Zone Layer Crossfade
Chain Volume
Left Handle
Right Handle
Toggle Button
```
