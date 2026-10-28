# Estabilidad y diagnóstico de 3.10.23

## Cambios de esta entrega

- Carga inicial de temas protegida frente a respuestas antiguas y borrados posteriores.
- Altura inicial del tray compartida con el dock. La animación no modifica la medida base.
- Espaciado/anclaje de tray estable por sesión de Explorer, monitor/DPI y escala explícita.
- `WM_SETTINGCHANGE` y cambios de color de Windows propagan transparencia a todas las superficies; una consulta inicial anterior no reemplaza el evento más reciente.
- Ajustes pide Acrylic a DWM en el hilo UI y comprueba el HRESULT. `accepted` significa petición aceptada por DWM, no prueba visual del efecto. Si falla/no es compatible se conserva fondo sólido legible. No se usa la confirmación de encolado de Tauri como prueba del compositor.
- Solicitudes de material serializadas y respuestas obsoletas ignoradas. El estado aparece en `data-native-material` para inspección técnica.
- Un mutex envenenado invalida el inventario vivo y reinicia el observador con espera acotada; conserva el contador para no revivir tokens antiguos.

## Recuperación y soporte

Menú contextual del icono propio de Bloom:

- **Restart system tray**: invalida inventario y vuelve a registrar el observador. No reinicia Explorer ni las aplicaciones del usuario. Si el worker no llegó a arrancar, usar **Restart Bloom**.
- **Copy Bloom diagnostics**: copia JSON local con versión, transparencia, estado del proveedor, cantidad de iconos y visibilidad/DPI de las superficies. No incluye títulos, rutas, nombres de aplicaciones, tooltips, ajustes ni el log. No se envía por red.
- **Quit Bloom** restaura la barra nativa como antes; sirve de recuperación si la integración no responde.

No se crean paneles ni se modifica la distribución aprobada. Un panel de diagnóstico, reordenación de iconos y notificaciones reales requieren entregas posteriores. Acrylic para dock/tray necesita una máscara independiente; no se activa sobre sus márgenes transparentes. En Windows sin soporte de backdrop DWM se usa el respaldo sólido.

## Dependencias revisadas antes de publicar

Se actualiza exclusivamente `rustls` 0.23.45 → 0.23.46 en Cargo.lock, parche revisado en PR #10. El árbol Windows confirma su uso por el actualizador HTTPS; no se integran actualizaciones masivas. Check/Clippy y 97 pruebas Rust vuelven a pasar tras el parche. La auditoría Bun solo cubre el lockfile JavaScript, no sustituye la revisión de Cargo.

Quedan dos alertas anteriores documentadas, sin ocultarlas: `glib` no aparece en el árbol de compilación Windows de esta app; `rand` 0.7.3 llega por `phf_generator` y la alerta exige un logger reentrante con características concretas. Bloom no define ese logger. Esto delimita el alcance observado, no declara la alerta corregida ni autoriza una actualización incompatible del árbol de Tauri.

## Medición reproducible de rendimiento

Con Bloom instalado en ejecución, ejecutar manualmente desde el repositorio:

```powershell
powershell -NoProfile -File scripts/measure-bloom.ps1 -Seconds 30
```

Repetir en reposo, con música y con bandeja abierta. El script solo lee el árbol Bloom/WebView2; informa CPU normalizada al total de procesadores lógicos y memoria privada total. Los procesos que nacen entre muestras no contribuyen CPU hasta tener dos lecturas; procesos que mueren entre lecturas pueden subestimarla. Comparar en condiciones iguales. No es una cifra de ahorro ni una medición del consumo de Explorer. No se instala ningún sondeo de rendimiento en segundo plano.

## Matriz de prueba instalada pendiente

| Caso | Resultado esperado |
| --- | --- |
| Actualizar 3.10.22 → 3.10.23 | Canal firmado y ajustes conservados |
| Reiniciar y cambiar tema mientras carga | Selección más reciente conservada, sin salto de tamaño del tray |
| Activar/desactivar transparencia de Windows | Cambia sin enfocar Bloom; texto sigue legible |
| Claro/oscuro y texto 100/130% | Superficies homogéneas, controles legibles |
| Maximizar/restaurar, Smart/Peek | Dock y bandeja aparecen/desaparecen coordinados |
| Captura de pantalla | Anclaje estable, escritorio vuelve a aceptar entrada |
| Slack, WhatsApp, OneDrive | Clic/doble clic/menú funcionan y quedan delante |
| Restart system tray | Iconos vuelven; no resucitan selecciones antiguas |
| Suspender/reanudar y Explorer reiniciado manualmente | Observador se recupera y se conserva acceso |
| 1080p/2K/4K y DPI mixto | Bordes coherentes y controles dentro de pantalla |
| Salir/reiniciar Bloom | Barra nativa restaurada, sin área transparente bloqueante |

Las pruebas automáticas y la publicación no sustituyen esta matriz. Los tests nativos que controlan Explorer/iconos siguen ignorados en la suite habitual.
