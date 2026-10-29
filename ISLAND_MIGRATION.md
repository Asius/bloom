# Transición a Bloom Island 3.11.0

## Resultado

Windows conserva su barra y bandeja originales. La aplicación compila solo tres ventanas: isla, volumen y Ajustes. Se eliminan fuentes y entradas de compilación del dock/bandeja, su inventario/clics, observador de Explorer, miniaturas/lanzadores/anclados y hooks Win+n. El menú del icono normal de Bloom permanece dentro de la bandeja nativa para Ajustes/diagnóstico/reiniciar/salir.

No se incorporó código de TwoCode/Coucou ni sus numerosas integraciones. Se preservan las funciones útiles existentes de la isla y su diseño; las nuevas funciones se decidirán después de probar esta base.

## Actualizar desde 3.10.23

Usar Settings → About → Check for Updates. Se mantienen identidad del instalador, canal y clave pública del actualizador. El instalador anterior conserva su cierre/restauración antes de instalar.

En el primer inicio, si existe taskbar_hidden.flag de una ejecución anterior, legacy_taskbar.rs restaura el estado nativo y elimina el marcador. En uso normal no se toca la barra. No se crea otro marcador, no se ejecuta un watchdog y no se integra nada en Explorer.

island_settings.rs retira las preferencias de dock/tray y transforma la escala efectiva anterior en escala de isla. Antes de la primera escritura conserva settings-before-island.json una única vez. Tema, calendario, música, volumen, archivos y claves personales no relacionadas se conservan. Un JSON dañado no se sobrescribe. Los archivos antiguos de anclados/cache no se leen ni se borran; quedan como datos históricos del usuario. Importar ajustes antiguos normaliza los campos retirados sin reactivar las funciones.

Los informes antiguos de bandeja se archivan fuera del código en propuestas/analisis/archivo-2026-10-28; el historial Git y las etiquetas 3.10 siguen conservando la implementación anterior.

## Prueba instalada pendiente

1. Actualizar desde 3.10.23: barra nativa con Inicio, ventanas e iconos de Windows disponible; no aparece dock/tray propio.
2. Reiniciar, salir y cambiar modo Fixed/Smart/Peek: la barra nativa conserva su estado, incluida su preferencia de autoocultación.
3. Reloj, calendario, música/visualizador, abrir fuente multimedia, volumen/mezclador y arrastrar archivos/texto funcionan.
4. Escala de isla y texto independientes; Ajustes no cambia de tamaño con la isla.
5. Maximizar/restaurar, pantalla completa y Recortes: isla no bloquea entrada; vuelve tras capturar.
6. Tema/transparencia y cambio de DPI, suspender/reanudar y varios monitores.
7. Copiar diagnóstico desde el icono normal: no expone títulos/rutas, no incluye el proveedor retirado.

La revisión de código, las suites y un navegador con IPC simulado se registran por separado; no equivalen a completar esta matriz en la instalación del propietario.
