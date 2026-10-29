# Estabilidad de vistas — 3.11.1

## Problema y resultado

El propietario reportó música y controles superpuestos al cambiar entre ambos; reiniciar recuperó la vista. En el navegador se reprodujo la coexistencia transitoria de sus dos nodos en ocho cambios rápidos. El bloqueo persistente de la captura no se reprodujo en una sesión nativa.

Las vistas tenían colas de salida independientes: el reproductor anterior seguía montado mientras aparecía el panel nuevo. Ahora una clave cambia todo el contenido de la isla y desmonta la vista anterior inmediatamente. Se mantienen los resortes del contenedor y el fundido de entrada; la salida del contenido ya no retrasa su retirada. Una única selección determina tanto geometría como contenido, incluso al terminar/desactivar una sesión multimedia.

Los actualizadores de estado multimedia quedan puros. La carátula deriva del mismo snapshot, y los cambios de permiso de búsqueda/fecha de posición ya no se descartan como duplicados. El visualizador compartido deja de importar App y no modifica el array recibido. Los controles multimedia reciben nombres accesibles.

## Regresión en navegador

Después de `bun run build`, ejecutar `node scripts/preview-island.mjs` y:

```powershell
node node_modules/vite/bin/vite.js preview --outDir src-tauri/target/ui-preview --host 127.0.0.1 --port 4321
```

Abrir `http://127.0.0.1:4321/?theme=light`. Los botones inferiores simulan eventos nativos; no controlan aplicaciones reales. La vista generada está en target/ui-preview; dist y el instalador no reciben estos mocks. Para otra variante usar `theme=dark&layout=compact&scale=1.3&font=1.3`.

1. Start media → Open music → rueda sobre la isla para abrir controles.
2. Pointer left → Open music → rueda; repetir rápidamente. Alternar también las flechas de calendario/controles.
3. Pause media y End media, incluyendo activarlos con teclado sin retirar el puntero de la isla.
4. Comparar clásico/compacto, temas y escalas. Cada muestra DOM debe contener una sola vista data-island-view y como máximo uno de expanded-music-container, command-center-content-minimal, calendar-content o shelf-content.
5. Comprobar pausa/siguiente, volumen y que Media seekable actualiza la capacidad del reproductor sin cambiar el título.

La comprobación de navegador usa IPC simulado. El propietario debe repetir la secuencia con su reproductor real, actualizar desde 3.11.0 y comprobar Smart/Peek, capturas y suspensión; no declarar completada esa validación por las pruebas puras o por publicar.

## Distribución

Un solo instalador NSIS de Windows x64. La firma .sig y latest.json son necesarios para el actualizador; ZIP/TAR de fuentes los genera GitHub. Las versiones antiguas y sus MSI se mantienen como historial. No se compilan instaladores de otros sistemas ni MSI en las nuevas entregas.
