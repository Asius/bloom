# Apariencia de Bloom Island

Tema compartido por isla, volumen y Ajustes. Superficies claras 243/249 y oscuras 32/44, acento de Windows y opacidad configurable. Cambios de tema/transparencia se sincronizan por eventos; las respuestas iniciales antiguas no sustituyen cambios recientes.

La isla y volumen comparten bloom-notch-scale; bloom-font-scale afecta solo al texto y Ajustes no toma la escala de isla. Segoe UI, radios, contraste de acento, foco y movimiento reducido son compartidos. Los controles retirados del dock/tray no aparecen en Ajustes.

Acrylic se solicita directamente a DWM para Ajustes y se conserva fondo sólido si no está disponible. Un HRESULT correcto prueba aceptación, no resultado visual. La isla conserva su acabado CSS y región de entrada con curvas; la paleta aproxima Windows, sin afirmar equivalencia exacta.

Las pruebas de navegador usan IPC simulado y no prueban compositor, entrada nativa ni recuperación del sistema. Matriz instalada en ISLAND_MIGRATION.md.
