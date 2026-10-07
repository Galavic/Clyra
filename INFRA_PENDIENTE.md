# Infra pendiente del rebrand a Clyra

El código ya dice Clyra en todas partes. Dominio real: `clyra-cli.xyz`
(landing en Vercel). Estado a 2026-10-07: release `v0.2.83` publicado en
`Galavic/Clyra` con todos los instaladores.

## 1. Dominio y edge (Cloudflare)

- `edge/wrangler.jsonc` — zona `zeron.sh`, rutas `edge.zeron.sh`,
  `zeron.sh/install.sh`, `zeron.sh/releases/*` (más las legacy `comet.*`).
  Worker `comet-native-edge`: NO renombrar (el storage de los Durable
  Objects está ligado al nombre; renombrarlo huérfana sesiones/rooms).
- `edge/src/install.sh` — ✅ resuelve versión por API de GitHub y descarga
  de `github.com/Galavic/Clyra/releases/download` (sin `BASE`, sin R2).
- `apps/clyra/src/main.rs:82` — `DEFAULT_EDGE_URL`
  (`https://edge.zeron.sh`).
- `apps/ios/Clyra/Views/SignInView.swift:13` y
  `apps/ios/Clyra/App/AppModel.swift:73` — `https://edge.zeron.sh`
  por defecto.
- `apps/landing/public/downloads.js` — ✅ apunta a GitHub Releases
  (con fallback pineado); `telemetry.js` acepta origen GitHub.
- `README.md`, `README.zh-CN.md` — ✅ ejemplo
  `curl -fsSL https://clyra-cli.xyz/install.sh | sh`.
  (Nota: `install.sh` vive en el repo; hay que publicarlo en esa URL
  o cambiar el ejemplo a la descarga directa de GitHub.)
- R2 `comet-native-releases` + `latest.txt`/`manifest.json`
  (ver `.github/workflows/release.yml`): el workflow sigue subiendo ahí
  ADEMÁS del GitHub Release. Mantener como espejo o retirar el paso R2
  cuando `clyra-cli.xyz/releases/*` sirva los archivos.
- `apps/landing/wrangler.jsonc` + `apps/www-redirect/` — ✅ jobs
  eliminados de `deploy.yml`: la landing vive en Vercel, Cloudflare no
  debe desplegarla. El redirect www→apex se hace en el dashboard de Vercel.

## 2. WorkOS (auth)

- `apps/clyra/src/main.rs:88` — `DEFAULT_WORKOS_CLIENT_ID` es el
  proyecto de Zeron. Crear proyecto WorkOS de Clyra y sustituir
  (o forzar por entorno `CLYRA_WORKOS_CLIENT_ID`).

## 3. GitHub / org

- Deps git en `Cargo.toml`: `zeronsh/zui`, `zeronsh/gpui-component`
  (funcionan tal cual; mover a fork propio solo si se quiere
  independencia total).
- ✅ `.github/FUNDING.yml` — `github: Galavic`.
- `.github/workflows/testflight.yml` — consulta `sh.zeron.ios`;
  actualizar al bundle nuevo.
- ✅ Enlaces `github.com/zeronsh/...` en `apps/landing/public/index.html`
  y badge DeepWiki del `README.md` → `Galavic/Clyra`. En `docs/` y
  `crates/theme/src/builtins.rs` se dejan como atribución de procedencia.
- `docs/research/*` y comentarios `zeronsh/comet#95`: historial,
  no tocar.

## 4. iOS / distribución Apple

- Bundle IDs ya renombrados en código a `sh.clyra.app` /
  `sh.clyra.ios*`: registrarlos en Apple Developer (el `teamID`
  de `testflight.yml` es el de Zeron).
- Keychain iOS usa el nuevo servicio: sesiones guardadas con el
  servicio viejo no migran (aceptado en el rebrand completo).
- `apps/ios/Clyra/Assets.xcassets/AppIcon.appiconset/AppIcon1024.png`
  ya es el icono Clyra (regenerar `.icns` al empaquetar, como siempre).

## 5. Windows

- `dist/windows/clyra.ico` + `clyra.rc` ya integrados (`build.rs`).
- El instalador que se use debe registrar el esquema `clyra://`
  (`x-scheme-handler/clyra` ya está en `dist/clyra.desktop` para Linux;
  en Windows no había registro en código tampoco antes).
- `scripts/package-windows.ps1` recibe `-ReleasesUrl`: pasar la URL
  del feed nuevo al empaquetar.

## Ya resuelto (no tocar)

Binario `clyra`, crates `clyra-*`, UI/cadenas/menús/título, logo `>_*`,
accent morado `#a78bfa` por defecto, scheme `clyra://`, bundle IDs en
código, `%LOCALAPPDATA%\Clyra` + `~/.clyra` con migración desde Zeron,
`CLYRA_*` con fallback `ZERON_*`, `clyra.service` + `sh.clyra.app`,
`clyra-update.json`, `Clyra.app`/`.dmg`/`.zip`, `ClyraTests`, iconos
dist/landing/iOS, READMEs y docs de uso.
