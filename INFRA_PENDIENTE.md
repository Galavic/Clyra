# Infra pendiente del rebrand a Clyra

El código ya dice Clyra en todas partes. Esto es lo que sigue atado a la
infraestructura de Zeron y hay que mover cuando se acabe la app.
Sustituir `<TU_DOMINIO>` por el dominio real en cada punto.

## 1. Dominio y edge (Cloudflare)

- `edge/wrangler.jsonc` — zona `zeron.sh`, rutas `edge.zeron.sh`,
  `zeron.sh/install.sh`, `zeron.sh/releases/*` (más las legacy `comet.*`;
  decidir si se mantienen para enlaces viejos). Worker `comet-native-edge`
  (renombrar o no: solo etiqueta interna).
- `apps/landing/wrangler.jsonc` — `zeron.sh`, `comet.zeron.sh`.
  Worker `comet-landing` (solo etiqueta interna).
- `apps/www-redirect/` — `www.zeron.sh → zeron.sh`; pasar a
  `www.<TU_DOMINIO> → <TU_DOMINIO>`.
- `edge/src/install.sh` — `BASE` por defecto `https://zeron.sh`
  (ya acepta `CLYRA_BASE_URL`, y `ZERON_BASE_URL` como fallback).
- `apps/clyra/src/main.rs:82` — `DEFAULT_EDGE_URL`
  (`https://edge.zeron.sh`).
- `apps/ios/Clyra/Views/SignInView.swift:13` y
  `apps/ios/Clyra/App/AppModel.swift:73` — `https://edge.zeron.sh`
  por defecto.
- `apps/landing/public/downloads.js:2` — base
  `https://zeron.sh/releases/` (y los href fijos de `index.html`,
  más `telemetry.js` que filtra por origen).
- `README.md:14`, `apps/clyra/src/update_cli.rs:71` — ejemplo
  `curl -fsSL https://zeron.sh/install.sh | sh`.
- R2 `comet-native-releases` + `latest.txt`/`manifest.json`
  (ver `.github/workflows/release.yml`): publicar ahí o mover bucket
  y re-apuntar el feed (`CLYRA_RELEASES_URL` / `zeron-update.json`
  en los ZIP sirve como override por release).

## 2. WorkOS (auth)

- `apps/clyra/src/main.rs:88` — `DEFAULT_WORKOS_CLIENT_ID` es el
  proyecto de Zeron. Crear proyecto WorkOS de Clyra y sustituir
  (o forzar por entorno `CLYRA_WORKOS_CLIENT_ID`).

## 3. GitHub / org

- Deps git en `Cargo.toml`: `zeronsh/zui`, `zeronsh/gpui-component`
  (funcionan tal cual; mover a fork propio solo si se quiere
  independencia total).
- `.github/FUNDING.yml` — `github: zeronsh`.
- `.github/workflows/testflight.yml` — consulta `sh.zeron.ios`;
  actualizar al bundle nuevo.
- Enlaces `github.com/zeronsh/...` en `apps/landing/public/index.html`,
  `README.md` (badge DeepWiki), `docs/` y `crates/theme/src/builtins.rs`
  (procedencia; esos se pueden dejar como atribución).
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
