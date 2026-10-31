# Bloom Island — base de escritorio

Decisión del propietario del 28 de octubre de 2026: dejar la barra y bandeja de Windows a Windows y concentrar Bloom en una isla interactiva útil.

Se conservan el diseño y animaciones del notch, reloj/calendario, medios y visualizador, volumen/mezclador, archivos arrastrados, temas y actualización firmada. Clima y widgets actuales siguen siendo opcionales. No se agregan agentes, nube, dictado, mascotas ni integraciones de las referencias en esta entrega.

Se retiran dock, lanzadores/anclados, miniaturas de ventanas, accesos Win+n, bandeja personalizada, lectura de iconos y estados de Explorer, DLL del observador, reservas inferiores y ocultación/vigilancia de barra nativa. También desaparecen sus controles, escalas, IPC y pruebas exclusivas. La isla y el volumen comparten escala; texto se configura aparte y Ajustes conserva su tamaño propio.

Los modos Fixed, Smart y Peek no reservan una franja de trabajo. Fixed se muestra sobre ventanas maximizadas y se oculta ante pantalla completa; Smart se oculta si una ventana se superpone y Peek se muestra al acercarse al borde superior. Ningún modo altera el estado ni la posición de la barra de Windows durante uso normal. Existe un icono normal de Bloom en la bandeja nativa para abrir Ajustes y salir. El comportamiento nativo con aplicaciones, monitores y DPI distintos requiere prueba instalada.

Compatibilidad antigua y pruebas: [ISLAND_MIGRATION.md](ISLAND_MIGRATION.md). No se declara ahorro de CPU/RAM sin una medición comparable. La reducción de módulos/sondeos es verificable por código, no sustituye un benchmark.
