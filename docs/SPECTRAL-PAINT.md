# Spektral målning

Måla dämpning direkt på ett spårs spektrogram — RX-reparation utan att lämna DAW:en. Verktyget
ligger i spårets verktygsrad: **🎨 Spektral målning**.

## Så fungerar det

1. Spårets ljud analyseras till ett spektrogram: STFT med 1024-samplers Hann-fönster och 75 %
   överlapp. Nivån är dBFS, normaliserad så en sinus med amplitud A hamnar på A·N/4 — samma
   regel som den spektrala kompressorn använder.
2. Bilden är **högst 1024 × 256 rutor**, oavsett hur långt ljudet är. Varje ruta är maxvärdet
   över de analysrutor den täcker, så en kort transient syns även när vyn är grov. Rad 0 är
   högsta frekvensen.
3. Penseln skriver en **förstärkning per (bildruta, bin)** i masken. Dämpningen är i dB,
   0 = orörd och −60 = nästan borta. Målningen syns som ett mörkt lager ovanpå spektrogrammet.
4. **Verkställ** kör masken över spåret: STFT, binvis förstärkning, ISTFT med överlapp-addering,
   och spårets buffer byts först när något faktiskt målats.

## Mätt

| Egenskap | Resultat |
| :--- | :--- |
| Tom mask rekonstruerar insignalen | största fel < 1e-3 (Hann² vid 75 % överlapp summerar till 1,5) |
| Målad dämpning kring 1 kHz | 1 kHz ned minst 30 dB, 4 kHz ändras < 1,5 dB |
| Spektrogrammets bin för 1 kHz vid 48 kHz | bin 21, nivå −6 dB för amplitud 0,5 |
| Vyns storlek | ≤ 1024 × 256 även för två minuters ljud |
| Fönstret i en headless egui-bildruta | ritas, texturerna skapas |

## Gränser

- **Ingen ångra efter Verkställ.** Spårets buffer byts, som `Ta bort klick`; målningen är
  avsiktlig och ska inte kunna försvinna av misstag — men den går inte att backa. Spara först
  om du är osäker.
- **Förspel:** `Mask::apply` lägger en framruta på N sampel innan analysen, annars är de första
  ~768 samplen för svaga (överlapp-adderingen är bara exakt där alla fyra fönstren bidrar).
  Latensen kompenseras bort — utsignalen har samma längd som insignalen.
- **Ingen separat kanal:** analysen är L+R och masken gäller båda. En målning tar samma sak ur
  båda kanalerna, vilket är rätt för reparation och fel för kreativ stereomålning.
- Masken lever bara så länge fönstret är öppet. Stängs det kastas målningen, och inget ljud rörs.
