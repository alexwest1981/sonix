const REPO = 'https://github.com/alexwest1981/sonix'
const LATEST = `${REPO}/releases/latest`
const RAW = 'https://raw.githubusercontent.com/alexwest1981/sonix/master'
const VERSION = 'v0.9.0'
const BASE = '/sonix'

function Waveform({ color, delay = '0s' }: { color: string; delay?: string }) {
  return (
    <div className="waveform" style={{ '--wave-color': color, '--wave-delay': delay } as React.CSSProperties} aria-hidden="true">
      {Array.from({ length: 34 }).map((_, index) => (
        <span key={index} style={{ height: `${18 + ((index * 19) % 47)}%` }} />
      ))}
    </div>
  )
}

function Brand() {
  return (
    <span className="brand">
      <span className="brand-mark"><i /><i /><i /><i /><i /></span>
      <span>SONIX <b>STUDIO</b></span>
    </span>
  )
}

const SHOTS = [
  { file: '03_piano_roll.jpg', alt: 'The piano roll with notes, velocity and scales', caption: 'Piano roll: note lengths, velocity per step and scales that keep the key.' },
  { file: '06_effects_mixer.jpg', alt: 'The mixer with channel strips and peak meters', caption: 'The mixer with channel strips, meters and effects.' },
  { file: '02_channel_rack.jpg', alt: 'The channel rack with 16 steps', caption: 'Channel rack: 16 steps per bar, one channel per sound.' },
  { file: '04_vocal_studio_leads.jpg', alt: 'Vocal Studio with several takes', caption: 'Vocal Studio: several takes, comping without throwing anything away.' },
  { file: '11_stem_separator.jpg', alt: 'The stem separator', caption: 'The stem separator, with the DSP path as the default.' },
]

export default function Page() {
  return (
    <main className="site-shell" id="top">
      <nav className="nav container" aria-label="Main navigation">
        <a href="#top" aria-label="Sonix Studio home"><Brand /></a>
        <div className="nav-links">
          <a href="#features">Features</a>
          <a href="#skarmbilder">Screenshots</a>
          <a href="#vad-saknas">What is missing</a>
        </div>
        <a className="nav-cta" href="#ladda-hem">Download <span>↓</span></a>
      </nav>

      <section className="hero container">
        <div className="hero-copy">
          <div className="eyebrow"><span className="live-dot" /> Linux · Rust · open source</div>
          <h1>Your idea.<br /><em>On your machine.</em></h1>
          <p className="hero-text">
            Sonix is a DAW for Linux. It records from the microphone, arranges on the timeline,
            mixes with real meters and exports what you hear. The audio engine runs on cpal and
            ALSA, projects are saved to disk, and nothing leaves your machine unless you connect
            an AI service yourself.
          </p>
          <div className="hero-actions">
            <a className="primary-button" href={LATEST} target="_blank" rel="noreferrer">Download {VERSION} <span>↓</span></a>
            <a className="text-button" href={REPO} target="_blank" rel="noreferrer">Read the code <span>↗</span></a>
          </div>
          <div className="hero-note"><span>✦</span> 7.3 MB binary. Built and tested on Omarchy and Arch.</div>
        </div>
        <div className="hero-visual">
          <div className="glow glow-one" /><div className="glow glow-two" />
          <div className="screenshot-card">
            <div className="card-bar"><span className="window-dots"><i /><i /><i /></span><span>SONIX STUDIO / TIMELINE</span><span className="card-status">● REAL SCREENSHOT</span></div>
            <img src={`${BASE}/shots/01_tidslinje_arranger.jpg`} width={1600} height={900} alt="Sonix Studio with the timeline, colour-coded tracks and waveforms" />
          </div>
          <div className="floating-tag tag-bpm"><b>120.0</b><small>BPM</small></div>
          <div className="floating-tag tag-tracks"><span>04</span><small>TRACKS + MIC</small></div>
        </div>
      </section>

      <section className="signal-strip" aria-hidden="true"><div className="container signal-inner"><span>RECORD</span><Waveform color="#d9ff3f" /><span>MIX</span><Waveform color="#ff6b35" delay=".4s" /><span>EXPORT</span><Waveform color="#9e7bff" delay=".8s" /></div></section>

      <section className="download container" id="ladda-hem">
        <div className="section-intro">
          <div className="eyebrow">01 / Download</div>
          <h2>Three ways in.<br /><span>Take the one that fits.</span></h2>
          <p>The ready-made binary needs neither Rust nor a build step. If you want the newest code, build from source; it takes a few minutes.</p>
        </div>

        <div className="download-grid">
          <article className="download-card">
            <span className="feature-number">FASTEST</span>
            <h3>Ready-made binary</h3>
            <p>x86-64, glibc 2.35 or newer. Unpack and run. The archive has the binary, the manual, the changelog, the licences and a sha256 checksum.</p>
            <pre className="code-block"><code>{`curl -LO ${REPO}/releases/download/${VERSION}/sonix-0.9.0-x86_64-unknown-linux-gnu.tar.gz
sha256sum -c sonix-0.9.0-x86_64-unknown-linux-gnu.tar.gz.sha256
tar xzf sonix-0.9.0-x86_64-unknown-linux-gnu.tar.gz
./sonix`}</code></pre>
            <a className="card-link" href={LATEST} target="_blank" rel="noreferrer">All releases on GitHub ↗</a>
          </article>

          <article className="download-card">
            <span className="feature-number">NEWEST CODE</span>
            <h3>Guided installer</h3>
            <p>Asks for a language, checks dependencies, builds with Cargo and adds a launcher to the application menu.</p>
            <pre className="code-block"><code>{`bash <(curl -fsSL ${RAW}/install.sh)`}</code></pre>
            <p className="fineprint">If you already have a clone, <code>bash install.sh</code> in that folder is enough. After changing the code: <code>bash install.sh --refresh</code>.</p>
          </article>

          <article className="download-card">
            <span className="feature-number">SANDBOX</span>
            <h3>Flatpak</h3>
            <p>The same binary, packaged in the omapak catalogue. The app's configuration and library live in the sandbox under <code>~/.var/app/</code>.</p>
            <a className="card-link" href="https://omapak.org/app/io.github.alexwest1981.sonix" target="_blank" rel="noreferrer">omapak.org/app/io.github.alexwest1981.sonix ↗</a>
          </article>
        </div>

        <div className="requirements">
          <div>
            <h3>What you need</h3>
            <ul>
              <li>64-bit Linux with PipeWire, JACK or ALSA running. PipeWire works through its ALSA layer.</li>
              <li>To build it yourself: Rust stable (edition 2024) and a C toolchain.</li>
              <li>For Windows VST plugins: yabridge and Wine.</li>
              <li><code>unzip</code> is only needed by the Suno import.</li>
            </ul>
          </div>
          <div className="callout">
            <p>
              <b>The release is from 11 September 2026.</b> Since then master has gained microtonal
              tunings, a modulation matrix, a groove pool and four stretch modes, among other
              things. They are in the source, not in the binary. To try them, run the installer
              alongside it.
            </p>
          </div>
        </div>
      </section>

      <section className="features container" id="features">
        <div className="section-intro">
          <div className="eyebrow">02 / What it does</div>
          <h2>What works.<br /><span>And nothing that pretends to.</span></h2>
          <p>Every line here connects to the audio engine or the project state. The details come from the app's own status list.</p>
        </div>
        <div className="feature-grid">
          <article className="feature-card feature-wide">
            <span className="feature-number">01</span>
            <div>
              <h3>Multitrack timeline</h3>
              <p>Drag a clip's edges to extend, loop or trim it. The slice tool cuts, automation draws curves, and markers split the song into sections.</p>
            </div>
            <div className="mini-timeline">
              <div className="mini-label">VOCALS</div><div className="mini-bars"><i /><i /><i /><i /><i /></div>
              <div className="mini-label orange">DRUMS</div><div className="mini-bars orange-bars"><i /><i /><i /><i /><i /></div>
            </div>
          </article>
          <article className="feature-card">
            <span className="feature-number">02</span>
            <div className="feature-icon">◒</div>
            <h3>Channel rack and piano roll</h3>
            <p>A 16-step sequencer with its own channels, and a piano roll with note lengths, velocity, scales and recording from a MIDI keyboard.</p>
          </article>
          <article className="feature-card">
            <span className="feature-number">03</span>
            <div className="feature-icon">≡</div>
            <h3>Mixer with real meters</h3>
            <p>Faders, peak meters, solo, mute, pan and width per track, plus buses, sends and a master chain.</p>
          </article>
          <article className="feature-card">
            <span className="feature-number">04</span>
            <div className="feature-icon">◉</div>
            <h3>Vocal Studio</h3>
            <p>Several takes per track, comping and a harmonizer. Pitch editing runs offline on the recorded file, so what you hear is what gets saved.</p>
          </article>
          <article className="feature-card">
            <span className="feature-number">05</span>
            <div className="feature-icon">⌁</div>
            <h3>Stem separation</h3>
            <p>Split a mix into vocals, drums, bass and other with DSP in the standard build. The neural path is opt-in and downloads its model first.</p>
          </article>
          <article className="feature-card accent-card">
            <span className="feature-number">06</span>
            <h3>Open files, open licences</h3>
            <p>Rust and egui, MIT or Apache-2.0. No account, no subscription, no telemetry. Seven interface languages, switched live, all complete.</p>
            <a href={REPO} target="_blank" rel="noreferrer">See the code ↗</a>
          </article>
        </div>
      </section>

      <section className="shots container" id="skarmbilder">
        <div className="section-intro">
          <div className="eyebrow">03 / Screenshots</div>
          <h2>This is how it looks.<br /><span>Unedited.</span></h2>
          <p>The images come from the app itself. All 29 are in the <a href={`${REPO}/tree/master/screenshots`} target="_blank" rel="noreferrer">screenshots folder</a>.</p>
        </div>
        <div className="shot-grid">
          {SHOTS.map((shot, i) => (
            <figure className={`shot${i === 0 ? ' shot-wide' : ''}`} key={shot.file}>
              <img src={`${BASE}/shots/${shot.file}`} width={1600} height={900} loading={i === 0 ? undefined : 'lazy'} alt={shot.alt} />
              <figcaption>{shot.caption}</figcaption>
            </figure>
          ))}
        </div>
      </section>

      <section className="limits" id="vad-saknas">
        <div className="container">
          <div className="eyebrow">04 / What is missing</div>
          <h2>This is not finished yet.</h2>
          <p className="limits-intro">It is an early release, and you may as well know the limits before you download it.</p>
          <ul className="limits-list">
            <li>VST3, VST2 and CLAP load and play only in a build with <code>--features plugin-host</code>. LV2 exists only in the catalogue, and plugin windows need a real X server.</li>
            <li>The modular patcher evaluates nodes in creation order, so the cables have to follow the signal path. A few node names promise more than the DSP delivers.</li>
            <li>Neural stem separation ships without model weights. The model is downloaded separately and needs a licence that suits you.</li>
            <li>No Suno integration. The Suno import reads exported stem packs and uses the local generator.</li>
            <li>There is no video and no scripting API. Microtonal tuning and some other tools are on master and arrive in the next release.</li>
          </ul>
          <p className="limits-note">The full list is in the <a href={`${REPO}#-whats-left--how-far-from-real`} target="_blank" rel="noreferrer">README</a>, the plan in the <a href={`${REPO}/blob/master/ROADMAP.md`} target="_blank" rel="noreferrer">ROADMAP</a>.</p>
        </div>
      </section>

      <section className="numbers container">
        <div className="numbers-row">
          <div><b>708</b><span>automated tests</span></div>
          <div><b>0</b><span>warnings in all four builds</span></div>
          <div><b>29</b><span>screenshots in the repo</span></div>
          <div><b>7</b><span>interface languages</span></div>
          <div><b>337</b><span>commits</span></div>
        </div>
      </section>

      <footer className="footer container">
        <a href="#top"><Brand /></a>
        <p>A music studio for Linux, written in Rust. MIT or Apache-2.0, whichever you prefer.</p>
        <a href={REPO} target="_blank" rel="noreferrer">github.com/alexwest1981/sonix ↗</a>
      </footer>
    </main>
  )
}
