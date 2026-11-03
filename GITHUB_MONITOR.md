# Seguimiento de GitHub

El panel consulta, en modo de lectura, las últimas 50 ejecuciones de un repositorio público seleccionado mediante `propietario/repositorio`. Al abrir una fila consulta los trabajos y pasos de esa ejecución. Cada fila muestra su propio SHA: CI, CodeQL y Release pueden pertenecer a revisiones distintas. La consulta se actualiza cada dos minutos; el indicador compacto muestra una ejecución Release en curso. GitHub puede limitar las consultas sin autenticación. Los datos anteriores se indican como tales ante un error de red. La verificación de canal, descarga y firma aún no está integrada y figura como no comprobada.

La conexión a una cuenta aún no está implementada. Para habilitarla hará falta identificar o registrar una GitHub App propia de Bloom, habilitar Device Flow, definir permisos de lectura mínimos para repositorios y Actions, instalarla en los repositorios elegidos e integrar el almacenamiento del token cifrado con DPAPI. El identificador de cliente de una GitHub App no es un token; no se deben pegar tokens ni secretos en chats, configuración del proyecto, commits o informes. La conexión que usa Codex para GitHub es independiente y no se transfiere a Bloom.

Fuentes: [GitHub App y Device Flow](https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/generating-a-user-access-token-for-a-github-app), [ejecuciones](https://docs.github.com/en/rest/actions/workflow-runs) y [trabajos](https://docs.github.com/en/rest/actions/workflow-jobs).


## Registro confirmado — 2 de noviembre de 2026

Se registró **Bloom Florecer** en la cuenta Asius: https://github.com/settings/apps/bloom-florecer.
- App ID público: `5588585`.
- Client ID público: `Iv23ctoA6xuYRnqst4Nd` (no es un secreto).
- Device Flow activado, Actions y Metadata de solo lectura, webhook desactivado.
- Instalación limitada a Asius por decisión expresa del propietario; otras cuentas no pueden instalarla todavía.
- GitHub mostró registro exitoso y solicita generar una clave privada antes de instalar la App. No se generaron claves privadas ni secretos.
- El flujo Device Flow, su almacenamiento DPAPI y la selección de repositorios autorizados siguen pendientes de implementación. Registrar la App no conecta la cuenta en Bloom.
