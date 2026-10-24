# Soundtrap – gränssnittsritning

> **Källa och tillförlitlighet.** Insamlad av en Orca-agent (codex) 2026-10-24 på beställning.
> Avsnitten 1–6 vilar på Soundtraps egna hjälpsidor (svarar 403 mot ren `curl` — botvägg, inte
> döda sidor) och på publicerade skärmbilder. **En bild har jag själv kontrollerat**:
> `https://static.soundtrap.com/payload-media/image/Automation.b2bd5366-1920x1080.webp` — en
> beskuren studiobild som ger de mått som är märkta `[mätt]` nedan. Övriga pixelmått i rapporten är
> agentens uppskattning, inte mätning.

**Syfte:** visuell och beteendemässig referens för Sonix. Observationerna gäller Soundtraps webbstudio för musik på dator, inte mobilappen. Soundtrap kan ändra gränssnittet; exakta mått är uppskattade från exempelbilder och är inte officiella specifikationer. [avlett]

## 1. Skärmens anatomi

Soundtraps egen terminologiguide namnger projektfält, kompositionslinjal, kompositionsyta, loopar, kommentarer, effekter, zoom, metronom, transport, spårhuvud, regioner och track options. [belagt: https://support.soundtrap.com/hc/en-us/articles/5216945579794-Soundtrap-Terminology-and-Features-A-Complete-Guide]

Publicerade desktopbilder visar en smal toppremsa med projekt- och menykontroller, linjal över arrangemanget, fast spårkolumn till vänster och stor tidsyta till höger. Ljudbiblioteket/Loops öppnas som en panel, vilket minskar arrangemangsytans bredd. [belagt: https://lib.unb.ca/sites/default/files/media/documents/PodcastingWorkshopMarch5_24_0.pdf] [belagt: https://smithswood.co.uk/wp-content/uploads/2020/05/Music-Home-learning-activities-Year-9-and-10.pdf]

Transporten ligger längst ned i de äldre desktopexemplen. Den upptar en låg, horisontell remsa och lämnar arrangemanget mellan toppfält och transport. [belagt: https://www.mthebronmusic.org/uploads/8/7/8/4/8784370/recording_in_soundtrap_-_step_by_step.pdf] [belagt: https://blog.soundtrap.com/how-to-make-music-in-soundtrap/]

Följande skiss sammanfattar den klassiska datorvyn i källbilderna. Relativa bredder och höjder är avlästa/uppskattade, inte dokumenterade specifikationer. [avlett]

```text
┌──────────────────────────────────────────────────────────────────────┐
│ Projekt / File Edit Settings / spara, dela, ångra                    │  ~8%
├─────────────────────┬────────────────────────────────────────────────┤
│ SPÅR / kontroller   │ Taktlinjal + cykel/loopfält                    │
│ namn, R, automation │────────────────────────────────────────────────│
│ volym, solo, mute   │ Klipp / regioner och vågformer                 │
│                     │                                                │
│                     │ Tidslinje / arrangemang                        │
│                     │                                                │
│ + Add New Track     │                                                │
├─────────────────────┴────────────────────────────────────────────────┤
│ Volym · klocka · Rec · Stop · bakåt · Play · framåt · tonart · BPM   │
└──────────────────────────────────────────────────────────────────────┘
   ~20–25% spårkolumn                ~75–80% arrangemang
```

Spårhuvuden är kvar till vänster medan kompositionsytan skrollas horisontellt. [belagt: https://support.soundtrap.com/hc/en-us/articles/5216945579794-Soundtrap-Terminology-and-Features-A-Complete-Guide] [avlett]

Källorna visar paneler som öppnas ovanpå eller intill studion, bland annat Loop Library och instrument/presetväljare. De belägger inte en enda fast panelbredd eller att alla paneler kan fällas ihop på samma sätt. [belagt: https://smithswood.co.uk/wp-content/uploads/2020/05/Music-Home-learning-activities-Year-9-and-10.pdf] [belagt: https://support.soundtrap.com/hc/en-us/articles/5216945579794-Soundtrap-Terminology-and-Features-A-Complete-Guide] [avlett]

## 2. Transporten

Transporten ligger längst ned i klassiska desktopbilder. Ordningen som syns i en guide är mastervolym, tidsdisplay, inspelning, stopp, bakåt, spela, framåt, tonart, tempo, metronom och inställningar. [belagt: https://www.mthebronmusic.org/uploads/8/7/8/4/8784370/recording_in_soundtrap_-_step_by_step.pdf]

Soundtraps egen guide beskriver transporten som play/record/restart/stop och nämner mastervolym, tempo, tonart, metronom och count-in. [belagt: https://blog.soundtrap.com/how-to-make-music-in-soundtrap/]

Play/stop/record och navigering visas främst som ikoner. Tempo och tonart är värden/text; metronomen har en textstatus som exempelvis “Off”. Knappdimensioner är inte angivna. I exempelbilden är de små, jämnt placerade träffytor i samma verktygsrad. [belagt: https://www.mthebronmusic.org/uploads/8/7/8/4/8784370/recording_in_soundtrap_-_step_by_step.pdf] [avlett]

Cycle Bar/loopmarkör hör till tidslinjens övre del enligt terminologiguiden, inte till transportens kontrollgrupp. [belagt: https://support.soundtrap.com/hc/en-us/articles/5216945579794-Soundtrap-Terminology-and-Features-A-Complete-Guide]

## 3. Spår och klipp

Spårhuvudet visar spårikon och namn. Soundtraps ordlista namnger Record Enable, Automation, volymratt, Solo, Mute och Track Options som egna huvudkontroller. [belagt: https://support.soundtrap.com/hc/en-us/articles/5216945579794-Soundtrap-Terminology-and-Features-A-Complete-Guide]

Skärmbilder visar dessa kontroller samlade i vänsterkolumnen; R är en kompakt knapp och volym visas som “Vol”/ratt. Spåren har egna färgaccenter. Exakt radhöjd går inte att läsa ut till pixelprecision ur källorna. [belagt: https://trentradio.ca/resources/21tradio_programmerresource_audioediting_js.pdf] [belagt: https://lib.unb.ca/sites/default/files/media/documents/PodcastingWorkshopMarch5_24_0.pdf]

Klipp kallas “regions” i Soundtraps dokumentation. Bilder visar rektangulära färgade block med namn; ljudklipp kan visa vågform, medan instrument-/loopregioner ofta är enfärgade med etikett. Exemplen använder rött, magenta och lila i arrangemanget. [belagt: https://support.soundtrap.com/hc/en-us/articles/5216945579794-Soundtrap-Terminology-and-Features-A-Complete-Guide] [belagt: https://lib.unb.ca/sites/default/files/media/documents/PodcastingWorkshopMarch5_24_0.pdf] [belagt: https://smithswood.co.uk/wp-content/uploads/2020/05/Music-Home-learning-activities-Year-9-and-10.pdf]

En region kan visa en Edit-kontroll vid hovring. Regionen har loop- och ändhandtag; fadehandtag visas vid hovring i de officiella instruktionerna. [belagt: https://support.soundtrap.com/hc/en-us/articles/5216945579794-Soundtrap-Terminology-and-Features-A-Complete-Guide] [belagt: https://support.soundtrap.com/hc/en-us/articles/205505532-How-to-Fade-In-and-Out-Regions-in-Soundtrap]

Valda regioner kan markeras genom att dra en markeringsruta eller klicka regioner; Shift används för flerval i vissa redigeringsflöden. Källbilder visar “Edit” på regionen, men stödet beskriver inte konsekvent en särskild konturfärg för markerat klipp. [belagt: https://support.soundtrap.com/hc/en-us/articles/209355889-How-to-Copy-or-Remove-Multiple-Regions-in-Soundtrap] [belagt: https://support.soundtrap.com/hc/en-us/articles/205662531-How-to-Split-Regions-in-Soundtrap] [avlett]

## 4. Färg och typografi

De granskade källorna exponerar ingen CSS eller officiell färgpalett för studiovyn. Färgbeskrivningen här är därför avläst ur publicerade skärmbilder: mörkgrå/svart arbetsyta, mörkare paneler och starka röda, magenta och lila klipp. [belagt: https://lib.unb.ca/sites/default/files/media/documents/PodcastingWorkshopMarch5_24_0.pdf] [belagt: https://smithswood.co.uk/wp-content/uploads/2020/05/Music-Home-learning-activities-Year-9-and-10.pdf]

Texten i exempelbilderna är huvudsakligen vit eller ljusgrå på mörkt underlag. Kontrollnamn och spårnamn ser ut att ligga ungefär kring 11–14 px på en vanlig desktopskärmbild; detta är en visuell uppskattning, inte ett CSS-värde. Radavståndet följer kompakta verktygsrader och spårrader. [belagt: https://trentradio.ca/resources/21tradio_programmerresource_audioediting_js.pdf] [avlett]

Ingen exakt hexkod eller teckensnittsfamilj kunde beläggas. [avlett]

## 5. Interaktion

- Flytta och markera: dra regioner för att placera dem. Markera flera genom att dra över kompositionsytan; Shift kan lägga till regioner i urvalet. [belagt: https://support.soundtrap.com/hc/en-us/articles/209355889-How-to-Copy-or-Remove-Multiple-Regions-in-Soundtrap]
- Klippa/dela: placera spelhuvudet och använd Ctrl/Cmd+E, eller hovra regionen, välj Edit och sedan Split region. [belagt: https://support.soundtrap.com/hc/en-us/articles/205662531-How-to-Split-Regions-in-Soundtrap]
- Kopiera/klistra: högerklick på regionen ger Copy; högerklick i kompositionsytan ger Paste vid spelhuvudet. Kortkommandon finns också. [belagt: https://support.soundtrap.com/hc/en-us/articles/205663831-How-to-Copy-and-Paste-in-Soundtrap]
- Kontextmeny: högerklick på region öppnar redigeringsval; officiell dokumentation nämner bland annat Copy, Split region och Mute region. Hovringsläget visar också Edit. [belagt: https://support.soundtrap.com/hc/en-us/articles/205663831-How-to-Copy-and-Paste-in-Soundtrap] [belagt: https://support.soundtrap.com/hc/en-us/articles/360026375434-How-to-Mute-Regions-Within-the-Same-Track-in-Soundtrap] [belagt: https://support.soundtrap.com/hc/en-us/articles/205662531-How-to-Split-Regions-in-Soundtrap]
- Inspelning: välj spårtyp, aktivera record enable på spåret och tryck Record i transporten. Voice & Mic-spåret väljer ingångsenhet i spårinställningarna. [belagt: https://support.focusrite.com/hc/en-gb/articles/360004801819-How-to-set-up-a-Focusrite-interface-in-Soundtrap]
- Instrumentbyte: välj instrument via Add New Track och instrumenttyp, eller öppna presetmenyn för ett instrumentspår. Terminologiguiden bekräftar Instrument Presets Menu; källorna visar inte ett enhetligt arbetsflöde för att konvertera befintligt spår mellan alla typer. [belagt: https://support.soundtrap.com/hc/en-us/articles/5216945579794-Soundtrap-Terminology-and-Features-A-Complete-Guide] [belagt: https://edu.soundtrap.com/student-resources/]
- Menyer/paneler: terminologiguiden listar Project Menu/Shortcuts, Grid Menu, instrumentpresets, loopfilter, export, kommentarer och effekter. Exakt innehåll beror på vy och spårtyp. [belagt: https://support.soundtrap.com/hc/en-us/articles/5216945579794-Soundtrap-Terminology-and-Features-A-Complete-Guide]

## 6. Vad som gör att det känns som Soundtrap

Följande grepp har starkast stöd i källorna och kan tillsammans skapa igenkänning: [avlett]

1. Arrangemanget är huvudytan med en fast spårkolumn till vänster. [belagt: https://lib.unb.ca/sites/default/files/media/documents/PodcastingWorkshopMarch5_24_0.pdf]
2. Regioner har tydlig färgkodning, namn och vågform när innehållet är ljud. [belagt: https://trentradio.ca/resources/21tradio_programmerresource_audioediting_js.pdf]
3. Record Enable sitter i spårhuvudet; Record sitter separat i transporten. [belagt: https://support.focusrite.com/hc/en-gb/articles/360004801819-How-to-set-up-a-Focusrite-interface-in-Soundtrap]
4. Transporten är en samlad nederlist med tydliga ikonknappar och synliga tempo-/tonartsvärden. [belagt: https://www.mthebronmusic.org/uploads/8/7/8/4/8784370/recording_in_soundtrap_-_step_by_step.pdf]
5. Regionredigering använder hovringskontroll “Edit” och kontextmeny, men har också kortkommandon. [belagt: https://support.soundtrap.com/hc/en-us/articles/205662531-How-to-Split-Regions-in-Soundtrap]
6. Loopbiblioteket är en panel integrerad med arrangemanget; loopar kan läggas till och dras in. [belagt: https://smithswood.co.uk/wp-content/uploads/2020/05/Music-Home-learning-activities-Year-9-and-10.pdf]
7. Spårkontrollerna är kompakta: automation, volym, solo och mute bor nära spårnamnet. [belagt: https://support.soundtrap.com/hc/en-us/articles/5216945579794-Soundtrap-Terminology-and-Features-A-Complete-Guide]

Sonix bör inte kopiera ett löst färgsystem utan semantik; i Sonix bör spårfärg fortfarande kommunicera typ och klippstatus. [avlett] Sonix bör inte kopiera Soundtraps små klickytor blint, eftersom Sonix också körs som fristående egui-app och behöver fungera med mus och tangentbord i tätare produktionsflöden. [avlett] Sonix bör inte efterlikna abonnemangsmarkeringar eller låsta ljud som visuellt mönster; det löser inte arrangemangsproblem och skulle göra arbetsytan mindre tydlig. [avlett]

## 7. Översättning till Sonix

Sonix har redan 210 px spårkolumn, 46 px normal spårrad, 32 px linjal, 12 px centralmarginal och 8 px panelmarginal. Standardtypskalan är 10,5 / 12,5 / 14,5 / 18 / 24 px. [belagt: ./src/ui/app/arranger.rs] [belagt: ./src/ui/app/frame.rs] [belagt: ./src/ui/theme.rs]

Sonix grundpalett är mörk: bakgrund RGB(18,20,24), panel RGB(28,32,40), text RGB(240,244,250), dämpad text RGB(140,150,168), cyan RGB(0,215,255) och lila RGB(172,97,245). [belagt: ./src/ui/theme.rs]

Små, lokala justeringar med befintlig layoutmodell: [avlett]

| Del | Förslag | Varför |
|---|---:|---|
| Spårrad | 50 px i stället för 46 px | Ger plats för Soundtrap-liknande namnrad och kompakt kontrollrad. |
| Spårkolumn | 220 px i stället för 210 px | Passar namn plus record/automation/volym/Solo/Mute utan att stjäla mycket tidsyta. |
| Linjal | 30 px i stället för 32 px | Behåll tydliga taktmarkörer men frigör höjd. |
| Kortpadding | 6 px horisontellt, 4 px vertikalt | Ligger nära Sonix befintliga 8 px panelpadding men ger tätare kort. |
| Klippradie | 3 px behålls | Befintlig Sonix-radie följer den kantiga DAW-estetiken; räcker för regioner. |
| Spårnamn | 13 px (FONT_S 12,5) | Behåll nuvarande lilla textskala för kompaktitet. |
| Kontroll/sekundärtext | 11 px (FONT_XS 10,5) | Dämpar etiketter utan att gå under befintlig miniminivå. |
| Tempo och tidsdisplay | 14–15 px (FONT_M 14,5) | Gör läsbara värden tydliga i transportlisten. |
| Browserpanel | 240 px standard, min 190 px | Sonix nuvarande 200 px kan kännas smal för kategorier och ljudnamn. |
| Dockpanel | 400 px standard, min 320 px | Sonix 470 px standard tar relativt mycket av arrangemanget. |
| Accent | Behåll cyanfärgad vald spårram, använd separat liten färglist | Sonix palettens semantiska färger bör överleva; Soundtrap-likhet uppnås med kompakta regioner, Edit vid hovring och spårkontroller. |

### Mätt ur den verifierade studiobilden `[mätt]`

Bilden `Automation.b2bd5366-1920x1080.webp` (Soundtraps egen marknadsföring, beskuren
arrangemangsbild) gav: klipp **~120 px** höga med **10–12 px** hörnradie och vit etikett i
**13–15 px**, spårhuvud **82–86 px** med titel **18–20 px** och en cirkelrund spårikon på **~44 px**,
automatiseringsfil under klippet i spårets egen färg med låg opacitet, panelytor **#222227–#2a2a31**,
accentlila **#6b5cff**, cyan klippfärg **#57b8db**.

**Ändrat i Sonix 0.9.84 efter detta:** spårhuvudets hörnradie 3 → 8 px och spårnamnet 11 → 13,5 px
(14 px på mikrofonspåret). Klipphöjd och spårhuvudshöjd är **inte** ändrade — de kräver att
huvudets kontrollrader flyttas, och det är en egen ändring.

Tabellen ändrar bara mått och befintliga komponenters utseende. Den förutsätter ingen ny ritmotor eller ombyggnad av appens huvudlayout. [avlett]

## Vad jag inte kunde belägga

- Exakta pixelmått för Soundtraps paneler, spårrader, transportknappar och marginaler. [avlett]
- Soundtraps CSS-hexvärden, exakta typsnitt och typografisk skala. [avlett]
- Om dagens webbstudio har samma nedertransport och färgpalett som de äldre publicerade desktopbilderna; flera tydliga bilder är från guider och tredjepartsmaterial. [avlett]
- En universell metod för att byta instrument på ett befintligt spår; källorna styrker instrumentval och presetmeny, men inte alla konverteringsfall. [avlett]
- En specifik färg/kontur för markerat regionblock i alla lägen. [avlett]
- Fullständigt innehåll i varje högerklicksmeny. Källorna visar bara de alternativ som behövs för respektive instruktion. [avlett]
- Någon av Soundtraps hjälpsidor som inte svarade med innehåll kunde inte verifieras vidare; rapporten bygger på sökindexerade officiella artiklar och publicerade skärmbilder. [avlett]
