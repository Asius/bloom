# Calendario de Bloom y Google Calendar

## Qué funciona hoy

La isla tiene navegación de meses, selección de día, botón Hoy y teclado
(flechas, Inicio/Fin, RePág/AvPág; Mayús cambia de año). Muestra los festivos
nacionales de Colombia y una selección de conmemoraciones, sin Internet ni
cuenta. El calendario usa español de Colombia; Hoy sigue la fecha del PC.

La conexión opcional permite seleccionar un cliente OAuth de escritorio y autorizar
la lectura en el navegador del sistema. Se guarda la autorización protegida con
DPAPI para el usuario de Windows. El propietario confirmó el inicio de sesión en
la aplicación instalada. La nueva consulta muestra los eventos del calendario
principal del día seleccionado, en la zona horaria del equipo; su prueba con
una cuenta real en la versión instalada queda pendiente. No activa notificaciones.

## Crear el cliente y conectar la cuenta

1. Accede con tu cuenta a [Google Cloud Console](https://console.cloud.google.com/).
2. Crea o elige un proyecto para Bloom. En APIs y servicios → Biblioteca,
   habilita **Google Calendar API**.
3. En Google Auth Platform configura nombre de la aplicación y correo de
   soporte. Para pruebas personales, configura la audiencia **External** en
   modo **Testing** y añade tu propio correo en **Test users**. Los nombres
   de las secciones pueden cambiar; sigue la documentación oficial enlazada.
4. En **Clients → Create client**, elige **Desktop app / Aplicación de
   escritorio**. No elijas una aplicación web ni una cuenta de servicio.
5. Descarga el JSON de ese cliente y consérvalo en una carpeta privada fuera
   del repositorio. No lo subas a GitHub. No necesitamos tu contraseña, códigos
   de verificación, access tokens ni refresh tokens por este chat.
6. En el calendario de Bloom elige «Conectar Google Calendar», selecciona ese
   JSON y autoriza el permiso en tu navegador. La app no solicita tu contraseña.
   Para eliminar las credenciales locales, usa «Eliminar conexión local»; para
   revocar además el permiso concedido, retira Bloom en la configuración de tu
   cuenta de Google. El cierre del navegador puede tardar hasta tres minutos en
   detectarse si no se completa la autorización.

No aceptes pagos o una suscripción solo para seguir esta guía. Si Google pide
condiciones nuevas o una configuración que no conoces, revisa ese paso primero.

## Contrato y límites de la integración

- OAuth de escritorio, navegador del sistema, PKCE S256, parámetro state
  impredecible y callback loopback temporal en 127.0.0.1. Sin login en WebView.
- Autorización inicial: `calendar.events.readonly`. No crea, edita ni borra.
  La selección de calendarios, si se incorpora, requerirá revisar sus permisos.
- Cancelar, denegar permisos, revocarlos o quedarse sin conexión no bloquea
  el calendario local. Si falla la consulta, se muestra un error y un reintento.
- Tokens en el backend, protegidos para el usuario Windows mediante DPAPI,
  fuera de settings.json, localStorage, logs y exportaciones.
- Eliminar conexión borra el archivo protegido local. Revocar el permiso en
  Google es un paso separado. No hay caché de eventos personales en esta fase.
- Se consulta el calendario principal en un intervalo acotado, con paginación y
  expansión de recurrentes; se excluyen cancelados. Se respetan los eventos de
  todo el día con final exclusivo. Las horas se presentan en la zona del equipo;
  si una hora no incluye desplazamiento compatible, se avisa y no se adivina.
  No se guardan eventos en disco ni se realiza sincronización en segundo plano.
  Un festivo presente también en Google puede aparecer junto al festivo local.
- Los avisos del notch se implementarán después, con elección del usuario,
  privacidad de títulos y exclusiones; conectar Calendar no los activa.
- Un cliente en modo Testing puede requerir volver a autorizar con frecuencia;
  verificar las reglas vigentes de Google al probar/publicar. Una app pública
  con scopes sensibles puede necesitar verificación adicional.

Referencias oficiales:
[OAuth para escritorio](https://developers.google.com/identity/protocols/oauth2/native-app),
[consulta de eventos](https://developers.google.com/workspace/calendar/api/v3/reference/events/list),
[scopes de Calendar](https://developers.google.com/workspace/calendar/api/auth),
[buenas prácticas](https://developers.google.com/identity/protocols/oauth2/resources/best-practices).

## Datos de Colombia y límites

Los 18 festivos se calculan con las reglas nacionales de la
[Ley 51 de 1983](https://www.funcionpublica.gov.co/eva/gestornormativo/norma_pdf.php?i=4954):
fechas fijas, traslado al lunes y fechas ligadas a Pascua. No es una lista
descargada de Google. Se conservan celebraciones que coinciden en fecha.
La UI admite 2002–2099 aplicando las reglas actuales, sin prometer que futuras
reformas o decretos locales estén incluidos. No incluye días cívicos municipales.

Las conmemoraciones no se presentan como descansos nacionales. La selección
inicial incluye Mujer, Madre Tierra, Medio Ambiente, Paz y Derechos Humanos
([ONU](https://www.un.org/en/observances/list-days-weeks)); Afrocolombianidad
([Ley 725/2001](https://www1.funcionpublica.gov.co/eva/gestornormativo/norma.php?i=4598));
Memoria y Solidaridad con las Víctimas desde 2011
([Ley 1448/2011, artículo 142](https://www.unidadvictimas.gov.co/wp-content/uploads/Documentos/ley/Ley_1448_2011.pdf));
y el 11 de noviembre histórico de Cartagena
([Alcaldía de Cartagena](https://cartagena.gov.co/noticias/cartagena-conmemora-211-anos-historia-heroismo-cultura-libertad)).
Es una selección pequeña y revisable; no un catálogo exhaustivo de efemérides.

EasyUI aporta la base de interacción visual; véase [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
