# Apariencia compartida de Bloom

Los temas integrados usan superficies neutras claras (243/249) y oscuras (32/44), opacidad configurable y el acento le铆do de Windows. Los controles HTML reciben color-scheme para que sus men煤s nativos sigan el tema. Las superficies comparten Segoe UI, radios de borde/panel/control y saturaci贸n del material desde appearance.css. El color sobre el acento se elige por luminancia para mantener contraste.

## Correcciones de esta revisi贸n
- Interruptores sin verde fijo; usan el acento del sistema y un indicador con contraste.
- Miniaturas y etiquetas del dock siguen el tema activo.
- Transici贸n CSS del interruptor v谩lida y foco visible para teclado.
- Filas de acciones de Ajustes accesibles con Enter/Espacio.
- Un resultado tard铆o del tema adaptativo no sustituye una selecci贸n posterior.
- La petici贸n de Acrylic se env铆a al cambiar su estado, evitando repetirla en cada movimiento de opacidad.
- Cambios de contraste forzado y transparencia reducida se aplican en vivo.

## Validaci贸n y l铆mites
82 pruebas frontend, TypeScript y Vite. Navegador integrado sobre la compilaci贸n, con IPC simulado: cambio claro/oscuro, acento de prueba, esquema de controles y texto al 130% sin desbordamiento horizontal en la vista probada. Capturas en propuestas/imagenes/2026-10-27-homogeneidad-oscuro.png y 2026-10-27-homogeneidad-claro-texto130.png, fuera del repositorio de c贸digo.
La simulaci贸n no valida Acrylic nativo ni el compositor de escritorio. La lectura real de preferencias y acento, los men煤s de apps, el DPI y maximizar/restaurar requieren prueba instalada. Acrylic contin煤a limitado a Ajustes; dock/tray conservan sus curvas y fondos CSS mientras se resuelve la m谩scara nativa. La paleta aproxima Windows; no afirma equivalencia exacta con Explorer.

## Carga inicial protegida
Los listeners de tema se instalan antes de pedir el snapshot. Los cambios o borrados recibidos durante esa carga conservan prioridad por clave; desmontar la vista impide aplicar la respuesta tard韆. El control del espejo local vuelve a comprobar las claves protegidas despu閟 de esperar la lectura del archivo. Ajustes compara revisiones por petici髇, de modo que una carga nueva (por ejemplo tras importar) acepta el archivo y solo descarta valores superados durante esa petici髇.
Validaci髇 adicional: respuestas antiguas, borrados durante espera, cancelaci髇, importaci髇 posterior y claves sin cambios cubiertas autom醫icamente. No equivale a una prueba del orden de eventos en la app instalada.
