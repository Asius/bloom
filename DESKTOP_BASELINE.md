# Base personal de escritorio

El perfil actual está preparado para el PC de Asius: dock, bandeja de Windows,
hora/calendario, sesiones multimedia de Windows, visualizador y volumen. Se
conservan las animaciones y los modos Fixed/Smart/Peek del dock y del notch.

## Funciones retiradas

Temporizador y sonido de finalización; batería y ahorro de energía; Wi-Fi y
Bluetooth; controles/HUD de brillo; conmutador DND que solo alteraba estado local;
accesos del notch a bandeja y centro nativo de notificaciones. Se retiran también
sus estados, consultas, suscripciones, comandos nativos y módulos sin consumidores.
La detección de capacidades para otros equipos se pospone a otra entrega.

El canal de anuncios del creador original y sus enlaces ya no se cargan. Se
eliminó su dominio de la CSP y la carga de fuentes de Google. Los scripts siguen
limitados a recursos locales. Las actualizaciones siguen usando exclusivamente
el canal firmado de `Asius/bloom` con la clave pública existente. La licencia y la
atribución original permanecen en [ATTRIBUTIONS.md](ATTRIBUTIONS.md).

## Límites entre módulos

- `SystemTray.tsx` presenta los grupos promovido/oculto y solicita acciones por
  identidad. `tray.rs` y `tray/model.rs` revalidan el destino nativo; nunca adivinan
  el mensaje privado de una aplicación ni ejecutan una posición antigua sin
  comprobarla. `tray/surface.rs` mantiene rectángulos y disponibilidad con vencimiento.
- El área de clic de la bandeja usa el origen actual de la ventana, independiente
  del rectángulo de apertura del dock central. La flecha presenta el catálogo ya
  disponible mientras se actualiza en segundo plano.
- `capture_blocks_bloom` en `services.rs` distingue el editor de Capturas de la
  selección de pantalla. El editor normal, incluso maximizado, no debe suspender
  Bloom. Mientras hay selección se cede la entrada; una revisión periódica permite
  recuperar la interfaz aunque falte un evento de Windows.
- Ajustes usa su propia ventana de 620×480 unidades lógicas y no aplica la escala
  de Bloom. El escalado de pantalla/DPI de Windows sigue siendo gestionado por Tauri.
- `statusWidgets.ts` normaliza la configuración compartida: elimina batería,
  identificadores desconocidos y duplicados antes de renderizar. Importar o
  conservar ajustes anteriores no vuelve a crear las funciones retiradas.

## Comprobación antes de distribuir

Ejecutar suites Bun y Rust, build frontend, check/Clippy, revisión del diff y
simulación de las vistas modificadas. CI y CodeQL deben pasar en el commit que
se etiqueta; después se verifican manifiesto, hashes y firmas del instalador.

La simulación no prueba clics de aplicaciones reales. El propietario verifica
flecha e iconos de bandeja con Capturas abierta, Slack/WhatsApp tras cerrar con X,
menús de OneDrive/Google Drive, captura por selección, escala de Ajustes, música,
volumen y restauración de la barra nativa al salir. Hasta su confirmación esta
base es una entrega para validación instalada, no una etapa de estabilidad cerrada.
