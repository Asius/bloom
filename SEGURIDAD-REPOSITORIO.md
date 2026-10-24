# Seguridad de Asius/bloom

Configuración revisada el 24 de octubre de 2026. Se conserva el push directo a `main`; el propietario autorizó al agente a gestionar push, PR y publicaciones. GitHub Desktop continúa disponible y sincronizado con esa rama. Las reglas de GitHub se guardan en el servidor; clonar el código no las reproduce automáticamente.

## Protecciones activas en GitHub

| Control | Configuración comprobada |
| --- | --- |
| Historial de `main` | Ruleset activo `main - conservar historial`: bloquea borrado y force push, sin actores exentos. Permite push normal; no exige PR ni checks previos. |
| Etiquetas `v*` | Ruleset activo `versiones - conservar etiquetas`: bloquea modificación, borrado y force push de etiquetas existentes, sin actores exentos. Permite crear una versión nueva. |
| Acciones externas | Se exige un SHA completo para cada acción. Esto fija su código; no garantiza que ese código sea seguro. |
| PR de forks externos | Sus workflows necesitan aprobación para todos los colaboradores externos. Revisar el cambio antes de aprobar la ejecución. |
| Token de Actions | Lectura de contenido/paquetes por defecto; desactivada la opción de crear/aprobar PR con Actions. Los permisos declarados por cada job pueden ampliar ese valor. |
| Dependencias | Grafo, alertas de Dependabot, alertas de malware, PR de parches de seguridad y actualizaciones de versiones habilitados. Se conserva la agrupación de `.github/dependabot.yml`. No hay integración automática de sus PR. |
| Secretos | Secret Protection y push protection encendidos. Detectan patrones compatibles; no reconocen necesariamente cualquier clave, incluida una clave de firma personalizada. |
| Código | Workflow CodeQL existente habilitado para JavaScript/TypeScript, Actions y Rust, con ejecución semanal y en cambios a `main`/PR. |
| Reportes privados | Habilitados en este fork; el enlace de `SECURITY.md` apunta a Asius/bloom. |
| Colaboradores | La página de acceso mostraba cero colaboradores añadidos. Esto no sustituye revisar por separado aplicaciones, tokens y claves autorizados en la cuenta. |

Consultar las reglas en [Settings → Rulesets](https://github.com/Asius/bloom/settings/rules), [Actions](https://github.com/Asius/bloom/settings/actions) y [Advanced Security](https://github.com/Asius/bloom/settings/security_analysis).

## Medidas guardadas en el código

- Todos los checkouts usan `persist-credentials: false`: el token no queda disponible como credencial Git para los pasos posteriores. Los pasos que lo necesitan lo reciben explícitamente.
- Release instala el frontend con `bun install --frozen-lockfile` y declara lectura por defecto. Su job conserva `contents: write`, necesario para publicar instaladores y el manifiesto del actualizador.
- CI, Release y CodeQL mantienen las acciones fijadas por SHA. Dependabot propone cambios revisables a esas referencias.
- `.gitignore` excluye archivos habituales de claves, `.env` locales y bundles de recuperación. No elimina secretos ya registrados ni reemplaza una revisión del diff.
- Las tres automatizaciones de IA heredadas (`AI triage`, `AI triage digest`, `opencode`) permanecen desactivadas en GitHub. Sus jobs requieren además la variable `BLOOM_ENABLE_AI_AUTOMATIONS=true`; no se ha creado esa variable. `opencode` exige también que el comentario provenga del propietario. Su instalador externo y sus permisos deben revisarse antes de una futura activación.

Los cambios de archivos comienzan a usarse en GitHub tras el push. Las reglas del servidor ya están activas; la autorización de gestión no las desactiva.

## Flujo de trabajo y recuperación

1. Revisar el diff, ejecutar las comprobaciones pertinentes y crear el commit con Summary y descripción breve.
2. El agente puede enviar `main`; el propietario también puede usar **Push origin** en GitHub Desktop. CI y CodeQL analizan después de subir: no bloquean previamente un push directo. Comprobar la revisión exacta antes de publicar una versión.
3. Revisar Actions y las alertas antes de publicar un instalador. Una alerta o PR de Dependabot necesita revisión; no implica que deba integrarse sin pruebas.
4. Para deshacer un cambio publicado, crear un commit de reversión. No usar force push ni mover una etiqueta ya publicada. Una corrección distribuida debe tener una versión nueva.
5. Mantener una copia Git recuperable fuera de `bloom/`. `git bundle create <ruta-nueva>.bundle --all` guarda el historial y referencias locales; comprobarla con `git bundle verify <ruta>.bundle`. Se puede clonar ese bundle en una carpeta nueva y vacía. Un bundle no incluye archivos sin commit, secretos externos, configuración del servidor, issues, PR ni instaladores de Releases.

## Límites

Estas medidas reducen errores y vías de abuso, pero no certifican ausencia de código malicioso. Al conservar push directo, quien tenga permiso de escritura puede introducir código sin revisión previa. Las reglas tampoco impiden a un administrador cambiar la configuración o borrar el repositorio completo. Mantener protegida la cuenta (2FA o passkey y revisión de accesos) y conservar copias independientes sigue siendo necesario.

La clave de firma del actualizador continúa como secreto del repositorio, con copia privada local. Alguien con permiso para modificar workflows podría intentar usar ese secreto. No se ha trasladado a un entorno con aprobación ni se ha cambiado la clave: esas medidas requieren diseñar y probar aparte la publicación. La protección de etiquetas no vuelve inmutables los archivos de una Release.

Referencias: [reglas disponibles](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/available-rules-for-rulesets), [permisos de Actions](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/enabling-features-for-your-repository/managing-github-actions-settings-for-a-repository), [uso seguro de Actions](https://docs.github.com/en/actions/reference/security/secure-use).
