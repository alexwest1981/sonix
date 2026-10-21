/** @type {import('next').NextConfig} */
const nextConfig = {
  // GitHub Pages serverar rena filer: ingen Node-server, ingen bildoptimering.
  output: 'export',
  // Projekt-sida: https://alexwest1981.github.io/sonix/
  basePath: '/sonix',
  trailingSlash: true,
  typescript: {
    ignoreBuildErrors: true,
  },
  images: {
    unoptimized: true,
  },
}

export default nextConfig
