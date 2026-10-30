# Avatar Bloom

El personaje aprobado vive junto a la hora y conserva la silueta ondulada del logo. SVG local sin dependencias, imágenes remotas, cuentas ni agentes.

- Reposo: parpadeos limitados y descanso después de 30 segundos sin interacción con el avatar ni música.
- Curiosidad: mirada horizontal sobre el propio botón; también responde al foco de teclado.
- Saludo: clic, Enter o Espacio; no propaga el clic a la navegación de paneles.
- Música: balanceo breve al comenzar la reproducción; después se queda quieto. El reproductor ampliado conserva su espacio.
- Descanso: ojos cerrados; vuelve a responder al entrar o pulsarlo.

Usa el tema y la escala de la isla. Las animaciones se desactivan con movimiento reducido o mientras la isla está oculta. Los temporizadores se limpian al desmontar; no hay seguimiento global del ratón ni bucle de sondeo. Las futuras funciones delegadas quedan fuera de esta entrega.

Validación: build TypeScript/Vite, 33 tests frontend y 33 Rust, check/Clippy, auditoría JS y prueba de interfaz con IPC simulado. El navegador confirma saludo de teclado, música, regreso al estado compacto, descanso y acceso a controles/calendario. Esto no prueba WebView2 instalado, DPI ni modos Smart/Peek reales.

Prueba del propietario: actualizar desde 3.11.2; comprobar saludo sin abrir panel, hora/calendario y música; dejarlo descansar, probar temas/escalas y ocultación Smart/Peek; verificar que la barra nativa sigue disponible.
