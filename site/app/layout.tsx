import type { Metadata, Viewport } from 'next'
import './globals.css'

export const metadata: Metadata = {
  title: 'Sonix Studio - DAW för Linux, skriven i Rust',
  description:
    'Spela in, arrangera, mixa och exportera på Linux. Öppen källkod i Rust och egui, 7,3 MB färdig binär, inget konto och ingen telemetri.',
  openGraph: {
    title: 'Sonix Studio - DAW för Linux',
    description: 'Öppen källkod i Rust och egui. Ladda hem binären eller bygg från källkoden.',
    url: 'https://alexwest1981.github.io/sonix/',
    siteName: 'Sonix Studio',
    images: ['https://alexwest1981.github.io/sonix/shots/01_tidslinje_arranger.jpg'],
    locale: 'sv_SE',
    type: 'website',
  },
}

export const viewport: Viewport = {
  colorScheme: 'dark',
  themeColor: '#11151b',
}

export default function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  return <html lang="sv"><body>{children}</body></html>
}
