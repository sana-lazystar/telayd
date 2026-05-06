# Project Rule: Telayd

> JARFIS sub-agent (특히 QA / DevOps)에 자동 주입되는 coding/operating rules.
> 이 파일이 비어있어도 JARFIS는 작동 (`/jarfis:project-init` Step 4.5의 empty 보장).
> SSOT는 `~/AI--SPILL-OVER/telayd-planning/mvp-scope/scope.md` v0.2.3.

---

## Rust (Daemon) Rules

1. **clippy `-D warnings` enforce** — `cargo clippy --workspace --all-targets -- -D warnings` 통과해야 PR merge.
2. **Unsafe code 금지** — `#![forbid(unsafe_code)]` crate-level. daemon 빌드에 unsafe 필요 없음.
3. **Allow directive 신중** — `#[allow(...)]` 사용 시 1줄 주석으로 이유 명시.
4. **Error handling**:
   - Daemon 경계 (CLI / WebSocket / hook IPC): `anyhow::Result<T>`로 error chain 보존.
   - Library 경계 (개별 모듈 public API): `thiserror::Error` derive로 명시적 error type.
   - Hook script 호출 결과: `Result<(), String>` (사용자 친화적 메시지).
5. **`tokio::spawn` 신중** — `JoinHandle` 보존하거나 cancellation token 명시. orphan task 금지.
6. **Logging**:
   - `tracing` crate 사용 (간단 macro: `info!`, `warn!`, `error!`, `debug!`).
   - Daemon 시작 시 `~/.config/telayd/logs/telayd-YYYYMMDD.log`에 file appender.
   - PII 로깅 금지 — token / hook payload content 로그 금지.

## TypeScript (PWA) Rules

1. **`strict: true` 모든 옵션 enforce** (`strictNullChecks`, `noImplicitAny`, `strictFunctionTypes`).
2. **`any` 금지** — `unknown`으로 시작 후 narrow.
3. **Biome `recommended: true`** + 추가 규칙: `useExhaustiveDependencies` (React hooks).
4. **WebSocket type safety** — `MessageEvent.data`는 `unknown`, parse 후 zod 또는 manual narrowing.
5. **No barrel `index.ts` exports** — Vite tree-shaking 효율 + cyclic dep 방지. 직접 path import.

## Hook Script (bash) Rules

1. **POSIX-compatible** — `#!/usr/bin/env bash` + `set -euo pipefail`. 단 macOS bash 3.2 (default)에서 작동 보장.
2. **shellcheck 통과** — `shellcheck -e SC2034 hook-script.sh` warning 0건.
3. **Daemon 부재 시 graceful exit** — `~/.config/telayd/daemon.sock` 존재 안 하면 `exit 0` (Claude Code timeout 막지 않도록).
4. **5초 timeout 안에 종료** — daemon에 IPC emit 후 즉시 `exit 0`. blocking 금지.

## Sentinel Protocol Rules

1. **L0 = sentinel 처리 안 함** — Sentinel Parser는 stub만 빌드 (모듈 슬롯 + trait, noop 구현).
2. **L1+ 활성 시** — spec v0.2.2 §4 format 그대로:
   ```
   <<TELAYD|status|msg=<text>|pct=<0-100>>>
   ```
3. **ANSI strip + whitespace strip + bracketed paste markers 처리** — spec §4.2~4.4.

## Acceptance Criteria 의무

scope.md §3.4의 21개 acceptance criteria를 빌드 결과로 **모두 검증**:
- 빌드 후 `cargo test --workspace` + manual 7일 dogfooding.
- **Anthropic GitHub issues #29214/#29438/#34581/#35637 시나리오** explicit test case (manual 또는 integration test).
- p95 latency < 500ms (daemon log timestamp 기반 측정).

## Security Rules

1. **Pairing token plaintext OK (local only)** — `~/.config/telayd/config.toml` (chmod 600). git ignore 필수.
2. **Daemon binding** — localhost:7777 (mobile-facing) + Unix socket (hook IPC). 외부 도달은 cloudflared만.
3. **No hardcoded secrets in code** — token은 OsRng + config.toml 저장.
4. **`.gitignore`** — `target/`, `node_modules/`, `dist/`, `~/.config/` 디렉토리 자체 (이건 user home이라 git에 안 들어감).

## Licensing & Distribution

1. **모든 release artifact에 LICENSE (FSL-1.1-MIT) + NOTICE 파일 동봉**.
2. **NOTICE에 cloudflared Apache 2.0 attribution 포함** (자동 download하는 binary).
3. **Anthropic AUP / Google ToS 준수** — Claude Code / Gemini CLI 자체는 bundle 안 함, 사용자가 본인 install.
4. **README에 Cloudflare transit 명시** — privacy 자명성.

## 작업 흐름 Rules (JARFIS workflow)

1. **트리거가 발생할 때 디렉토리 생성** — 빈 패키지 scaffold 미리 만들기 금지. L0 = `daemon/` + `pwa/`만.
2. **scope.md SSOT 변경 시 INDEX.md + 관련 cross-doc 같은 turn에 갱신** (telayd-planning 정신 동일).
3. **acceptance criteria 충족 전엔 L1 진입 brainstorm 시작 금지** — scope drift 가드레일.
