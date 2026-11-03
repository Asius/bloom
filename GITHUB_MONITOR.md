# Seguimiento de GitHub

El panel consulta ejecuciones de Actions, trabajos y pasos del repositorio activo. La selección guardada admite hasta 50 repositorios, pero el seguimiento y los avisos corresponden al activo; no hay vigilancia simultánea de los 50. Si el repositorio pertenece a la cuenta conectada o está en su selección autorizada, las consultas usan el token de usuario; las consultas públicas de otros repositorios siguen disponibles sin iniciar sesión. GitHub limita el acceso a los permisos de Bloom, la cuenta y los repositorios habilitados en la instalación.

## Conectar y elegir repositorios

En GitHub, la App **Bloom Florecer** usa Device Flow con su Client ID público. Bloom solicita solo `offline_access`, el único scope admitido por GitHub Apps; las capacidades reales las determinan los permisos de la App. Los códigos del dispositivo permanecen en memoria, los tokens no pasan a React ni a IPC y el acceso/renovación se guardan cifrados con DPAPI para el usuario actual de Windows. Los access tokens vencen y se renuevan con rotación del refresh token. Desconectar elimina el archivo local protegido; para revocar también en GitHub, hay que hacerlo desde los ajustes de aplicaciones autorizadas.

La lista se obtiene desde las instalaciones autorizadas de esa GitHub App y sus repositorios accesibles para el usuario. El backend exige tanto el ID de Bloom como el ID estable de la cuenta **Asius**, y valida la selección contra esa lista antes de guardarla. GitHub solo permite instalar esta App en Asius, según la decisión del propietario; el propietario aún debe completar la autorización de instalación desde su cuenta GitHub antes de que aparezcan repositorios privados. Bloom no necesita la clave privada de la App para el Device Flow de usuario.

«Autorizar repositorios de Bloom» abre la página fija de instalación de la App mediante un comando nativo limitado a la ventana principal. No depende de permisos generales del plugin opener en React. Si el navegador no se puede abrir se muestra el error; al volver con el puntero al panel se recarga la lista, y «Actualizar» también permite hacerlo manualmente. Si el panel se cerró, se consulta de nuevo al abrirlo. El inicio de sesión y la autorización de repositorios son pasos distintos: abrir la página no confirma que GitHub haya concedido el acceso.

## Canal y firma

«Comprobar» fuerza una consulta del canal updater `latest`, obtiene el instalador Windows x64 publicado y le pide al verificador updater de Tauri que valide su firma Minisign con la clave pública configurada. La operación descarga el instalador completo, lo verifica en memoria y lo descarta; no lo ejecuta ni instala. La interfaz indica verificado solo cuando termina esa comprobación criptográfica. Si latest no está disponible, la plataforma no coincide o la firma falla, se muestra el error.

La comprobación no instala ni confirma el estado de firma Authenticode de Windows; valida la firma Minisign usada por el actualizador Bloom. La publicación todavía debe comprobarse con CI, CodeQL, Release y artefactos de la versión exacta.

Fuentes: [Device Flow de GitHub Apps](https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/generating-a-user-access-token-for-a-github-app), [permisos de la cuenta e instalaciones](https://docs.github.com/en/rest/apps/installations), [renovación de tokens](https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/refreshing-user-access-tokens) y [verificación del artefacto Tauri updater](https://docs.rs/tauri-plugin-updater/2.12.0/tauri_plugin_updater/struct.Update.html#method.download).
