# Probar y publicar Bloom desde Asius/bloom (Windows)

Este fork reemplazará la instalación de Bloom original. Ambas versiones usan el
mismo nombre, identificador, configuración y bloqueo de instancia. Cierra Bloom
desde el icono de la bandeja antes de iniciar una compilación de desarrollo o
instalar un ejecutable nuevo. Verifica que la barra de tareas de Windows vuelva
a aparecer al salir.

## Desarrollo local

GitHub Desktop sirve para ver cambios, crear commits y enviarlos a GitHub; no
compila ni instala la aplicación. Para probar cambios de código sin crear un
instalador, instala Bun 1.4.0, Rust estable y los prerrequisitos de Tauri para
Windows. Desde esta carpeta:

```powershell
bun install
bun run tauri dev
```

La ventana de desarrollo se recompila mientras editas. Sal de ella antes de
iniciar la Bloom instalada. Para comprobar una compilación:

```powershell
bun run build
cd src-tauri
cargo check --locked
cargo test --locked
```

## Primera versión instalable del fork

1. La clave de firma de este fork ya se generó en
   `%USERPROFILE%\.tauri\bloom-fork.key`, fuera del repositorio y de OneDrive.
   Conserva una copia segura: perder la clave privada impide firmar futuras
   actualizaciones para las instalaciones existentes. No generes otra para
   este canal mientras haya instalaciones que confíen en su clave pública.
2. `plugins.updater.pubkey` y el endpoint de `src-tauri/tauri.conf.json` ya
   apuntan a la clave nueva y a `Asius/bloom`.
3. En GitHub, activa Actions para el fork. En **Settings →
   Secrets and variables → Actions**, crea los secretos
   `TAURI_SIGNING_PRIVATE_KEY` (contenido del archivo `.key`) y, solo si la
   clave se generó con contraseña, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. La
   clave preparada para este fork no tiene contraseña. No subas el archivo
   privado al repositorio.
4. Confirma y envía los cambios de configuración con GitHub Desktop. Aumenta
   la versión por encima de la Bloom instalada. Para publicar desde `main`, usa
   `bun run release X.Y.Z`. Este comando cambia los archivos de versión, crea
   el commit y el tag, y los envía. El flujo `Release` de GitHub Actions genera
   el instalador NSIS (`bloom_X.Y.Z_x64-setup.exe`), su firma y `latest.json`.
5. Comprueba que el flujo terminó correctamente y ejecuta
   `bun scripts/verify-updater.mjs vX.Y.Z`. Descarga el `.exe` desde
   **Releases** de `Asius/bloom`, cierra la Bloom original e instálalo una vez.

Antes del primer Release no hay un `latest.json` del fork que se pueda
verificar; la comprobación previa del canal se aplica desde el segundo Release.

## Actualizaciones posteriores

Enviar commits a GitHub **no** actualiza una instalación. Cada nueva versión
instalable requiere un número de versión nuevo, un tag y un Release firmado.
Desde la Bloom del fork, **Settings → About → Check for Updates** permite
comprobar e instalar una versión nueva de inmediato. La opción **Auto Update**
debe estar activada para instalaciones automáticas: el programa comprueba al
iniciar como máximo una vez cada 24 horas y espera que la versión publicada
tenga al menos 24 horas antes de instalarla por sí solo.

La instalación original apunta al repositorio original y confía en su clave.
Por eso no puede cambiar al canal `Asius/bloom` mediante su actualizador:
necesitas instalar manualmente el primer `.exe` de este fork.

Referencias: [Tauri updater](https://v2.tauri.app/plugin/updater/),
[instaladores Windows](https://v2.tauri.app/distribute/windows-installer/),
[GitHub Actions en forks](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows).
