import type { Metadata, Viewport } from 'next'
import './globals.css'

export const metadata: Metadata = {
  title: 'Sonix Studio - a DAW for Linux, written in Rust',
  description:
    'Record, arrange, mix and export on Linux. Open source in Rust and egui, a 7.3 MB binary, no account and no telemetry.',
  openGraph: {
    title: 'Sonix Studio - a DAW for Linux',
    description: 'Open source in Rust and egui. Download the binary or build from source.',
    url: 'https://alexwest1981.github.io/sonix/',
    siteName: 'Sonix Studio',
    images: ['https://alexwest1981.github.io/sonix/shots/01_tidslinje_arranger.jpg'],
    locale: 'en_US',
    type: 'website',
  },
}

export const viewport: Viewport = {
  colorScheme: 'dark',
  themeColor: '#11151b',
}

export default function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  return <html lang="en"><body>{children}</body></html>
}
