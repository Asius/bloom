# Notificaciones de Windows en Bloom

Bloom consulta las notificaciones de aplicaciones que Windows pone a disposición, después de que la persona active la función en Bloom y Windows permita el acceso. La cobertura y los permisos pueden variar por equipo y aplicación.

## Activar y consultar

1. Abre la isla y usa las flechas de panel hasta **Notificaciones**.
2. Pulsa **Activar**. Bloom guarda esa preferencia; no consulta contenido antes de la activación.
3. El panel agrupa avisos de la misma aplicación y remitente. Al desplegar un grupo se muestran sus mensajes individuales.
4. Cuando llega o cambia un aviso, Bloom conserva visible el avatar, el reloj y el punto azul a su derecha; debajo muestra el aviso durante cuatro segundos. El punto permanece mientras Windows tenga avisos disponibles. La consulta se actualiza aproximadamente cada cinco segundos.
5. El menú permite abrir la aplicación si Windows proporciona un identificador compatible, o quitar un aviso/grupo. **Borrar todo** usa la API de Windows para vaciar las notificaciones disponibles. Esas acciones también cambian el centro de notificaciones de Windows.
6. Para cerrar solo el aviso emergente, pulsa su **X**. Para dejar de consultar, pulsa la campana tachada en la cabecera; Bloom quita los contenidos de la memoria de la interfaz.

Si la instalación no puede solicitar o utilizar el permiso, Bloom informa que el acceso no está disponible y ofrece abrir los ajustes de privacidad de Windows. La preferencia de Bloom y el permiso de Windows son controles distintos: desactivar Bloom no revoca por sí solo el permiso de Windows. La posibilidad de solicitarlo cuando aún no se ha concedido está pendiente de validación y puede no estar disponible en el instalador actual.

## Privacidad y límites

- Bloom no conserva una copia persistente del contenido. Solo guarda la preferencia de activación y vuelve a consultar lo que Windows proporcione al iniciar.
- La agrupación visual se basa en aplicación y título/remitente. Los avisos sin título permanecen separados para evitar combinar contenido distinto.
- **Quitar** y **Borrar todo** retiran avisos del centro de Windows. El último limpia las notificaciones disponibles, no solo las visibles en Bloom.
- Windows no proporciona a esta API una acción genérica para marcar como leída. La apertura depende de que el aviso incluya un identificador de aplicación que Windows pueda iniciar; no se garantiza para todas las aplicaciones.
- Bloom no ofrece respuestas ni ejecuta las acciones privadas de cada notificación. Los iconos de aplicación siguen siendo genéricos.
- La lectura inicial no presenta todas las notificaciones antiguas como nuevas. Los cambios de contenido en un identificador ya existente también pueden activar el aviso breve.
- Los avisos de periféricos y actualizaciones conservan su comportamiento; el calendario no crea recordatorios.
- La sonda previa vio notificaciones con el permiso ya permitido en un equipo sin identidad de paquete. Faltan pruebas instaladas de lectura, concesión/revocación, retirada desde Bloom y Windows, reinicio, modos, ventanas maximizadas, DPI y escala. La simulación de navegador no demuestra comportamiento nativo de Windows.
