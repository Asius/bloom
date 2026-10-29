# Bloom Island

Una isla interactiva para Windows que complementa el escritorio. La barra de tareas y la bandeja nativas permanecen disponibles.

## Funciones de esta base

- Hora y calendario.
- Reproducción multimedia, visualizador y controles.
- Volumen y mezclador por aplicación.
- Espacio para archivos/texto arrastrados.
- Modos Fixed/Smart/Peek, temas claro/oscuro y escala de texto independiente.
- Actualización firmada desde [Asius/bloom](https://github.com/Asius/bloom/releases/latest).

Desde 3.11.1 se publica un único instalador Windows x64 `.exe` (NSIS), acompañado de firma y manifiesto para las actualizaciones. [Corrección de transiciones y prueba de interfaz](ISLAND_STABILITY.md).

La línea 3.11 retira el dock y bandeja propios de la línea 3.10. No requiere reinstalar ni reiniciar Explorer al usar la actualización integrada. Ver [migración y pruebas](ISLAND_MIGRATION.md).

## Desarrollo

Bun 1.4.0, Rust estable, Windows MSVC/SDK y WebView2. `bun install --frozen-lockfile`, después `bun run tauri dev`. Para comprobar: `bun test scripts`, `bun run build` y, dentro de src-tauri, `cargo check --locked`, `cargo clippy --locked --all-targets`, `cargo test --locked`.

Código principal en src/ y src-tauri/src/. Referencias, propuestas e informes del proyecto personal se mantienen fuera de este repositorio. [Guía del repositorio](AGENTS.md) y [publicación](RELEASE_CHECKLIST.md).

## Referencias y licencia

Fork personal de [Bloom de Sehajveer Singh](https://github.com/SehajveerSingh2005/bloom), con atribución y licencia GPL-3.0-only conservadas: [LICENSE](LICENSE).

Inspiración para futuras interacciones: [Coucou](https://github.com/Louis-CFM/coucou) y [TwoCode](https://www.twocode.nl/). No se incorpora su código ni se anuncian sus funciones como propias de esta versión.
