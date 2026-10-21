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
  { file: '03_piano_roll.jpg', alt: 'Pianorullen med noter, anslag och skalor', caption: 'Pianorullen: notlängder, anslag per steg och skalor som håller tonarten.' },
  { file: '06_effects_mixer.jpg', alt: 'Mixern med kanalstrippar och toppmätare', caption: 'Mixern med kanalstrippar, mätare och effekter.' },
  { file: '02_channel_rack.jpg', alt: 'Kanalracket med 16 steg', caption: 'Kanalracket: 16 steg per takt, egna kanaler.' },
  { file: '04_vocal_studio_leads.jpg', alt: 'Röststudion med flera tagningar', caption: 'Röststudion: flera tagningar, kompning utan att kasta något.' },
  { file: '11_stem_separator.jpg', alt: 'Stämsseparatorn', caption: 'Stämsseparatorn, med DSP-vägen som standard.' },
]

export default function Page() {
  return (
    <main className="site-shell" id="top">
      <nav className="nav container" aria-label="Huvudnavigation">
        <a href="#top" aria-label="Sonix Studio hem"><Brand /></a>
        <div className="nav-links">
          <a href="#features">Funktioner</a>
          <a href="#skarmbilder">Skärmbilder</a>
          <a href="#vad-saknas">Vad som saknas</a>
        </div>
        <a className="nav-cta" href="#ladda-hem">Ladda hem <span>↓</span></a>
      </nav>

      <section className="hero container">
        <div className="hero-copy">
          <div className="eyebrow"><span className="live-dot" /> Linux · Rust · öppen källkod</div>
          <h1>Din idé.<br /><em>På din maskin.</em></h1>
          <p className="hero-text">
            Sonix är en DAW för Linux. Den spelar in från mikrofonen, arrangerar på tidslinjen,
            mixar med riktiga mätare och exporterar det du hör. Ljudmotorn ligger i cpal och ALSA,
            projekten sparas på disk, och ingenting lämnar maskinen om du inte själv kopplar på en
            AI-tjänst.
          </p>
          <div className="hero-actions">
            <a className="primary-button" href={LATEST} target="_blank" rel="noreferrer">Ladda hem {VERSION} <span>↓</span></a>
            <a className="text-button" href={REPO} target="_blank" rel="noreferrer">Läs koden <span>↗</span></a>
          </div>
          <div className="hero-note"><span>✦</span> 7,3 MB färdig binär. Byggd och testad på Omarchy och Arch.</div>
        </div>
        <div className="hero-visual">
          <div className="glow glow-one" /><div className="glow glow-two" />
          <div className="screenshot-card">
            <div className="card-bar"><span className="window-dots"><i /><i /><i /></span><span>SONIX STUDIO / TIDSLINJEN</span><span className="card-status">● RIKTIG SKÄRMBILD</span></div>
            <img src={`${BASE}/shots/01_tidslinje_arranger.jpg`} width={1600} height={900} alt="Sonix Studio med tidslinje, färgkodade spår och vågformer" />
          </div>
          <div className="floating-tag tag-bpm"><b>120.0</b><small>BPM</small></div>
          <div className="floating-tag tag-tracks"><span>04</span><small>SPÅR + MIKROFON</small></div>
        </div>
      </section>

      <section className="signal-strip" aria-hidden="true"><div className="container signal-inner"><span>SPELA IN</span><Waveform color="#d9ff3f" /><span>MIXA</span><Waveform color="#ff6b35" delay=".4s" /><span>EXPORTERA</span><Waveform color="#9e7bff" delay=".8s" /></div></section>

      <section className="download container" id="ladda-hem">
        <div className="section-intro">
          <div className="eyebrow">01 / Ladda hem</div>
          <h2>Tre vägar in.<br /><span>Ta den som passar.</span></h2>
          <p>Den färdiga binären kräver varken Rust eller kompilering. Vill du ha det allra nyaste bygger du från källkoden, det tar några minuter.</p>
        </div>

        <div className="download-grid">
          <article className="download-card">
            <span className="feature-number">SNABBAST</span>
            <h3>Färdig binär</h3>
            <p>x86-64, glibc 2.35 eller nyare. Packa upp och kör. Arkivet innehåller binären, manualen, ändringsloggen, licenserna och en sha256-summa.</p>
            <pre className="code-block"><code>{`curl -LO ${REPO}/releases/download/${VERSION}/sonix-0.9.0-x86_64-unknown-linux-gnu.tar.gz
sha256sum -c sonix-0.9.0-x86_64-unknown-linux-gnu.tar.gz.sha256
tar xzf sonix-0.9.0-x86_64-unknown-linux-gnu.tar.gz
./sonix`}</code></pre>
            <a className="card-link" href={LATEST} target="_blank" rel="noreferrer">Alla utgåvor på GitHub ↗</a>
          </article>

          <article className="download-card">
            <span className="feature-number">NYASTE KODEN</span>
            <h3>Guide-installeraren</h3>
            <p>Frågar efter språk, kontrollerar beroenden, bygger med Cargo och lägger en genväg i startmenyn.</p>
            <pre className="code-block"><code>{`bash <(curl -fsSL ${RAW}/install.sh)`}</code></pre>
            <p className="fineprint">Har du redan en klon räcker <code>bash install.sh</code> i mappen. Efter en kodändring: <code>bash install.sh --refresh</code>.</p>
          </article>

          <article className="download-card">
            <span className="feature-number">SANDBOX</span>
            <h3>Flatpak</h3>
            <p>Samma binär, paketerad i omapak-katalogen. Appens konfiguration och bibliotek hamnar i sandlådan under <code>~/.var/app/</code>.</p>
            <a className="card-link" href="https://omapak.org/app/io.github.alexwest1981.sonix" target="_blank" rel="noreferrer">omapak.org/app/io.github.alexwest1981.sonix ↗</a>
          </article>
        </div>

        <div className="requirements">
          <div>
            <h3>Det som behövs</h3>
            <ul>
              <li>64-bitars Linux med PipeWire, JACK eller ALSA igång. PipeWire fungerar genom sitt ALSA-lager.</li>
              <li>För egen kompilering: Rust stable (edition 2024) och en C-toolchain.</li>
              <li>För Windows-VST: yabridge och Wine.</li>
              <li><code>unzip</code> behövs bara av Suno-importen.</li>
            </ul>
          </div>
          <div className="callout">
            <p>
              <b>Utgåvan är från 11 september 2026.</b> Sedan dess har master fått bland annat
              mikrotonala stämningar, en modulationsmatris, groove-pool och fyra sträcklägen. De
              finns i källkoden, inte i den färdiga binären. Vill du prova dem kör du
              installeraren bredvid.
            </p>
          </div>
        </div>
      </section>

      <section className="features container" id="features">
        <div className="section-intro">
          <div className="eyebrow">02 / Vad den gör</div>
          <h2>Det som fungerar.<br /><span>Och inget som låtsas.</span></h2>
          <p>Varje rad här är kopplad till ljudmotorn eller projektets state. Uppgifterna kommer från appens egen statuslista, inte från en önskelista.</p>
        </div>
        <div className="feature-grid">
          <article className="feature-card feature-wide">
            <span className="feature-number">01</span>
            <div>
              <h3>Tidslinje med multitrack</h3>
              <p>Dra i klippens kanter för att förlänga, loopa eller trimma. Slice-verktyget delar, automationen ritar kurvor och markörer delar låten i sektioner.</p>
            </div>
            <div className="mini-timeline">
              <div className="mini-label">VOCALS</div><div className="mini-bars"><i /><i /><i /><i /><i /></div>
              <div className="mini-label orange">DRUMS</div><div className="mini-bars orange-bars"><i /><i /><i /><i /><i /></div>
            </div>
          </article>
          <article className="feature-card">
            <span className="feature-number">02</span>
            <div className="feature-icon">◒</div>
            <h3>Kanalrack och pianorulle</h3>
            <p>16-stegssekvenserare med egna kanaler, och en pianorulle med notlängder, anslag, skalor och inspelning från MIDI-klaviatur.</p>
          </article>
          <article className="feature-card">
            <span className="feature-number">03</span>
            <div className="feature-icon">≡</div>
            <h3>Mixer med riktiga mätare</h3>
            <p>Faders, toppmätare, solo, mute, panorering och bredd per spår, plus bussar, sends och en masterkedja.</p>
          </article>
          <article className="feature-card">
            <span className="feature-number">04</span>
            <div className="feature-icon">◉</div>
            <h3>Röststudion</h3>
            <p>Flera tagningar per spår, kompning och harmonizer. Pitchredigeringen kör offline på den inspelade filen, så det du hör är det som sparas.</p>
          </article>
          <article className="feature-card">
            <span className="feature-number">05</span>
            <div className="feature-icon">⌁</div>
            <h3>Stämsseparering</h3>
            <p>Dela en mix i sång, trummor, bas och övrigt med DSP i standardbygget. Den neurala vägen är opt-in och hämtar sin modell först.</p>
          </article>
          <article className="feature-card accent-card">
            <span className="feature-number">06</span>
            <h3>Öppna filer, öppna licenser</h3>
            <p>Rust och egui, MIT eller Apache-2.0. Inget konto, ingen prenumeration, ingen telemetri. Sju gränssnittsspråk som byts live.</p>
            <a href={REPO} target="_blank" rel="noreferrer">Se koden ↗</a>
          </article>
        </div>
      </section>

      <section className="shots container" id="skarmbilder">
        <div className="section-intro">
          <div className="eyebrow">03 / Skärmbilder</div>
          <h2>Så ser den ut.<br /><span>Oredigerat.</span></h2>
          <p>Bilderna kommer från appen själv. Alla 29 ligger i <a href={`${REPO}/tree/master/screenshots`} target="_blank" rel="noreferrer">screenshots-mappen</a>.</p>
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
          <div className="eyebrow">04 / Vad som saknas</div>
          <h2>Det här är inte klart än.</h2>
          <p className="limits-intro">Det är en tidig utgåva, och gränserna är lika bra att känna till innan du laddar hem den.</p>
          <ul className="limits-list">
            <li>VST3, VST2 och CLAP laddas och spelas bara i ett bygge med <code>--features plugin-host</code>. LV2 finns bara i katalogen, och plugin-fönster kräver en riktig X-server.</li>
            <li>Modulära patchern räknar noderna i skapelseordning, så kablarna måste följa signalvägen. Några nodnamn lovar mer än DSP:n gör.</li>
            <li>Den neurala stämssepareringen skeppas utan modellvikter. Modellen hämtas separat och måste ha en licens som passar dig.</li>
            <li>Ingen Suno-integration. Suno-importen läser exporterade stämpaket och använder den lokala generatorn.</li>
            <li>Video och skript-API finns inte. Mikrotonalitet och några andra verktyg ligger på master och kommer i nästa utgåva.</li>
          </ul>
          <p className="limits-note">Hela listan står i <a href={`${REPO}#-whats-left--how-far-from-real`} target="_blank" rel="noreferrer">README</a>, planen i <a href={`${REPO}/blob/master/ROADMAP.md`} target="_blank" rel="noreferrer">ROADMAP</a>.</p>
        </div>
      </section>

      <section className="numbers container">
        <div className="numbers-row">
          <div><b>707</b><span>automatiska prov</span></div>
          <div><b>0</b><span>varningar i alla fyra byggena</span></div>
          <div><b>29</b><span>skärmbilder i repot</span></div>
          <div><b>7</b><span>gränssnittsspråk</span></div>
          <div><b>330</b><span>commits</span></div>
        </div>
      </section>

      <footer className="footer container">
        <a href="#top"><Brand /></a>
        <p>En musikstudio för Linux, skriven i Rust. MIT eller Apache-2.0, beroende på vad du föredrar.</p>
        <a href={REPO} target="_blank" rel="noreferrer">github.com/alexwest1981/sonix ↗</a>
      </footer>
    </main>
  )
}
