# Notificaciones de Windows en Bloom

Esta primera versión permite consultar en la isla las notificaciones de aplicaciones que Windows pone a disposición. Es una entrega para probar en la aplicación instalada; la cobertura y el permiso pueden variar entre sistemas y aplicaciones.

## Activar y consultar

1. Abre la isla y usa las flechas de panel hasta **Notificaciones**.
2. Pulsa **Activar**. Bloom guarda esa elección para volver a usarla al iniciar; no consulta contenido antes de la activación.
3. Si Windows ya permite el acceso, el panel muestra los avisos disponibles. Los nuevos pueden aparecer brevemente en la isla; un punto azul permite volver al panel. La consulta se actualiza aproximadamente cada cinco segundos.
4. Para desactivar la lectura en Bloom, pulsa el botón de campana tachada en la cabecera del panel. La función deja de consultar y elimina la lista de la memoria de la interfaz.

Si la instalación no puede solicitar o utilizar el permiso, Bloom informa que el acceso no está disponible y ofrece abrir los ajustes de privacidad de Windows. La opción de activación de Bloom y el permiso de Windows son controles distintos: desactivar Bloom no revoca por sí solo los permisos de Windows. La posibilidad de solicitar el permiso cuando aún no se ha concedido está pendiente de validación y puede no estar disponible en el instalador actual.

## Privacidad y límites

- Bloom refleja los avisos disponibles en Windows y no guarda una copia persistente de sus contenidos. Guarda solamente la preferencia de activación. Al reiniciar consulta de nuevo lo que Windows aún proporcione.
- Descartar un aviso desde Windows puede hacer que desaparezca de Bloom al actualizar la consulta. Bloom no elimina avisos de Windows.
- La lectura inicial presenta los avisos disponibles sin mostrarlos todos como nuevos. Se utilizan iconos genéricos en esta primera versión.
- La disponibilidad de contenido, iconos y eventos depende de cada aplicación y de Windows. No se prometen apertura de la aplicación, respuestas, filtros ni sincronización de descartes desde Bloom.
- Los avisos de periféricos y actualizaciones que Bloom ya mostraba conservan su comportamiento; no se ha creado un historial para ellos. El calendario no crea recordatorios.
- La sonda independiente obtuvo una lista no vacía sin identidad de paquete en un equipo con acceso ya permitido. Falta probar presentación, permisos, revocación, reinicio, modos de isla, ventanas maximizadas y escalas en la instalación real. Esos resultados no pueden inferirse de pruebas simuladas.
