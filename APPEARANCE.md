# Apariencia compartida de Bloom

Los temas integrados usan superficies neutras claras (243/249) y oscuras (32/44), opacidad configurable y el acento leído de Windows. Los controles HTML reciben color-scheme para que sus menús nativos sigan el tema. Las superficies comparten Segoe UI, radios de borde/panel/control y saturación del material desde appearance.css. El color sobre el acento se elige por luminancia para mantener contraste.

## Correcciones de esta revisión
- Interruptores sin verde fijo; usan el acento del sistema y un indicador con contraste.
- Miniaturas y etiquetas del dock siguen el tema activo.
- Transición CSS del interruptor válida y foco visible para teclado.
- Filas de acciones de Ajustes accesibles con Enter/Espacio.
- Un resultado tardío del tema adaptativo no sustituye una selección posterior.
- La petición de Acrylic se envía al cambiar su estado, evitando repetirla en cada movimiento de opacidad.
- Cambios de contraste forzado y transparencia reducida se aplican en vivo.

## Validación y límites
82 pruebas frontend, TypeScript y Vite. Navegador integrado sobre la compilación, con IPC simulado: cambio claro/oscuro, acento de prueba, esquema de controles y texto al 130% sin desbordamiento horizontal en la vista probada. Capturas en propuestas/imagenes/2026-10-27-homogeneidad-oscuro.png y 2026-10-27-homogeneidad-claro-texto130.png, fuera del repositorio de código.
La simulación no valida Acrylic nativo ni el compositor de escritorio. La lectura real de preferencias y acento, los menús de apps, el DPI y maximizar/restaurar requieren prueba instalada. Acrylic continúa limitado a Ajustes; dock/tray conservan sus curvas y fondos CSS mientras se resuelve la máscara nativa. La paleta aproxima Windows; no afirma equivalencia exacta con Explorer.
