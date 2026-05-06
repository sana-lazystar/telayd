# telayd-l0-kickoff Meeting Notes

**Date**: 2026-05-06
**Participants**: PO, TL, owner (이산하/sana-lazystar)
**Source brainstorms**: b-1 (vision + brand) → b-2 (MVP scope down) → b-3 (Transport + 사업화 + Tech) → **b-4 (사업화 NO + L0 dogfooding lock + 5-iteration audit cycle)**
**SSOT**: `~/AI--SPILL-OVER/telayd-planning/mvp-scope/scope.md` v0.2.3 (CONVERGED)

---

## Topic 1: 사업화 vs Portfolio 결정

### Discussion

- 시장조사 (5 카테고리 multi-agent 병렬 리서치) 결과 thesis-shifting 발견:
  - Anthropic Remote Control 출시 (2026-02-25, Claude.ai mobile 12.48M MAU 번들) — Telayd MVP commodity화
  - happy.engineering / claudecodeui 같은 무료 OSS 동일 architecture 이미 존재
  - Cursor agents PWA (2025-06-30) — Cursor cloud agent 한정
- 사업화 6개 결정 변수 검토:
  - (1) 회사 비전 — 약함, "캐시 좀 땡기는 정도" / 10년 비전 안 그려짐
  - (2) 재정 buffer — OK
  - (3) GTM 적성 — 견디는 편 (즐기지 않음)
  - (4) Anthropic 베팅 confidence — 약함 ("굳이 안 할 수도")
  - (5) 한국 polyglot CLI 5명 인터뷰 — **못 잡음** (decisive)
  - (6) LinkedIn JARFIS 시리즈 반응 — 좋아요 1-2 / 노출 몇백건 / DM/사내/헤드헌터 반응 0
- Portfolio thesis도 6번 답으로 약함 → "내가 쓸 도구 + JARFIS 약속 이행 + JARFIS upgrade testbed"가 진짜 동기로 명확화

### Agreements

- **사업화 NO** confirmed (decision #1)
- **Dogfooding-driven OSS only** — 외부 보상 의존 X
- **JARFIS testbed로서의 가치 우선** — Telayd 빌드 과정이 JARFIS 자체 검증/진화

### Open Items

- LinkedIn 글 publish timing — L0 done 시 / OSS release 시 (각 1편)

---

## Topic 2: L0/L1/L2 Phase 모델 + Module Matrix

### Discussion

- 단일 MVP cut이 너무 큼 → 3-phase 점진:
  - L0 = 본인 mac 1대 dogfooding spike
  - L1 = 지인 3-5명 OK (cross-CLI 시작)
  - L2 = b-1 vision 도달 (Skill DSL + multi-provider redundancy)
- 15 module × 3 level matrix 작성 (scope.md §2)

### Agreements (Module Matrix L0 cut)

L0 빌드되는 10개 모듈:
1. Hook Installer (Claude Code only, global settings.json)
2. Hook Script (bash ~50 LOC, Unix socket IPC)
3. Tmux Controller (send-keys + dialog ready detection)
4. Sentinel Parser (**stub만** — L0 noop, L1+ extensibility)
5. CF Tunnel Manager (quick tunnel, cloudflared 자동 install)
6. WebSocket Bridge daemon side (localhost:7777, 1 client)
7. WebSocket Client PWA side (basic reconnect)
8. PWA UI (페어링 + choice UI)
10. Pairing (hard-coded token, 32-byte URL-safe random)
15. Auto-update / 배포 — manual git pull

L0에서 빌드 안 함:
- 9. Desktop UI → CLI only (`telayd start`)
- 11. E2E Encryption → CF HTTPS만
- 12. Skill DSL
- 13. Multi-provider Redundancy
- 14. Output Validation

### Open Items

- L1 진입 trigger (acceptance 충족 / 부족함 누적 / 외부 demand)

---

## Topic 3: Architecture (L0)

### Discussion

- IPC 채널 분리 — mobile-facing wss vs hook IPC를 같은 채널로? 별도?
- Module dependency graph — 10 모듈을 어느 순서로 빌드?
- spec v0.2.2 §6 (5 daemon module) + §11 (Pluggable Transport) + §12 (Tech Stack) 그대로 적용

### Agreements

- **IPC 채널 분리**:
  - mobile-facing: localhost:7777 plain `ws://` (cloudflared가 public side에서 `wss://`로 termination)
  - hook IPC: Unix socket `~/.config/telayd/daemon.sock` (file permission 600)
- **Module Dependency Graph 5-layer** (scope §3.2.1):
  - Layer 1 Bootstrap (Hook Installer + CF Tunnel Manager + Build infra)
  - Layer 2 Daemon Core (WebSocket Bridge + Tmux Controller, ← Pairing token)
  - Layer 3 Hook Integration (Hook Script + Sentinel Parser stub, ← Layer 1+2)
  - Layer 4 PWA (WebSocket Client + PWA UI, ← Layer 2)
  - Layer 5 Integration test (end-to-end)
- **Architecture diagram** (scope §3.3 ASCII)

### Open Items

- Quick tunnel URL 변경 시 자동 재페어링 UX (§9 L0 open question — daemon이 새 URL을 stdout + macOS notification, mobile bookmark fragment 형태로 token 보관)
- localhost:7777 port collision 처리

---

## Topic 4: Acceptance Criteria (L0 Done Definition)

### Discussion

- "본인 7일 매일 사용"만으로는 wedge 검증 불가 → Anthropic 4 issues 시나리오 acceptance 추가 필요
- latency 측정 boundary 명확화 — mobile-side network 포함? inject-only?
- Build/Distribution acceptance와 licensing-audit §7 (L2 prerequisite) 충돌 해결

### Agreements (총 21개)

- **Functional 9개**: 7일 매일 사용 / 모든 prompt 렌더링 / 응답 inject / Anthropic #29214+#29438 / Anthropic #34581 (앱 열려있을 때 한정) / Anthropic #35637 / `--dangerously-skip-permissions` 무관 / 재페어링 < 60초 / reconnect 10초 이내
- **Quality 3개**: Round-trip inject latency < 500ms p95 (inject-only segment 측정) / 7일간 critical bug 0건 / minor bug ≤ 3건
- **Performance 1개**: daemon CPU < 5% idle, mem < 100MB
- **Security 4개**: Pairing token 32-byte URL-safe random / config gitignore / daemon localhost binding / hook script graceful exit
- **Build 4개**: cargo build / pnpm build / `telayd init` 단일 명령 / LICENSE placeholder OK (full attribution L2)

### Open Items

- 5-min hook async timeout 시 사용자 응답 wait — 잠정 (a) 그대로 둠 (native UI 무한 대기)

---

## Topic 5: JARFIS Handoff Strategy

### Discussion

- JARFIS v4 workflow 구조 파악 (work.md + project-init.md + work-meeting.md + meeting-artifacts.md)
- Sub-agent inject 채널 7가지 — `project-profile.md` / `project-context.md` / `project-rule.md` / `meetings/<name>/` (--meeting 플래그) / `$ARGUMENTS` / Phase 1a 답안 / wiki-cache (org만)
- `project-context.md`는 designed purpose가 codebase navigation 보조 (4-section: Codebase Structure / Conventions / Paths / Caveats) — vision/scope spec 박는 곳 X

### Agreements (Hybrid 구조)

- **scope.md SSOT** = `~/AI--SPILL-OVER/telayd-planning/mvp-scope/scope.md` (사용자 reading 편의)
- **JARFIS injection** = telayd repo의 `meetings/20260506-telayd-l0-kickoff/` 4 files (`--meeting telayd-l0-kickoff` 플래그)
- `.jarfis-project/project-profile.md` + `project-rule.md`만 manual 작성 (greenfield)
- `.jarfis-project/project-context.md`는 빌드 후 `/jarfis:project-update`로 자동 생성

### Phase 1a 답안 (lock)

| 질문 | 답 |
|---|---|
| `design.mode` | text (figma 없이 markdown spec) |
| `responsive` | pc-mobile (desktop CLI + PWA mobile) |
| `api.mode` | null (외부 BE 없음 — daemon이 backend) |
| `devops` | false (CI/CD 없음, 개인 사용) |
| PO extras | ux-direction = false, legal-review = false |

### JARFIS 호출 명령

```
/jarfis:work L0 데몬 prototype 본격 구현 — packages/daemon (Rust hook installer + tmux controller + WebSocket bridge + CF Tunnel manager) + packages/pwa (Vite + React 페어링/choice UI). 본인 mac 1대 working spike, 21 acceptance criteria 충족 시 done. --meeting telayd-l0-kickoff
```

---

## Topic 6: Audit Cycle 5-Iteration

### Discussion

- 사용자가 명시적으로 "정합성 무한 반복 audit" 요청
- 깨끗한 Opus 4.7 sub-agent에 컨텍스트 주입해서 정합성 체크 → fix → 재검증 cycle

### Iteration 결과

| Iter | NEW 발견 | Fix 적용 | scope.md version |
|---|---|---|---|
| 1 | CRITICAL 2 + MAJOR 7 + MINOR/NIT 13 + Cross-doc 5 | 22건 | 0.1.0 → **0.2.0** |
| 2 | CRITICAL 1 + MAJOR 5 + MINOR 9 | 15건 | 0.2.0 → **0.2.1** |
| 3 | MAJOR 5 (regression 2 포함) + MINOR 8 | 7건 | 0.2.1 → **0.2.2** |
| 4 | MINOR 9 (M1/M2/M9 fix, M3-M8 ship-NIT) | 3건 | 0.2.2 → **0.2.3** |
| 5 | **CONVERGED** — no NEW CRITICAL/MAJOR / no regressions / cross-doc consistent | — | 0.2.3 그대로 |

### Cross-doc fix 부산물

- `architecture/telayd-protocol.md` v0.2.1 → **v0.2.2** (b-4 사업화 NO 정정)
- `architecture/licensing-audit.md` (한국 법령 N/A 명시 / E2E timing L2 lock / 사업화 결정 align)
- `INDEX.md` 다수 staleness fix (Spec v0.2.2 / 사업화 N/A / b-1~b-4 / Transport L0 quick / L1 named)

---

## 핵심 결정 요약 (decisions.md 16개 참조)

1. 사업화 NO
2. L0 dogfooding lock
3. Time 무제한 + 완성도 최우선
4-5. Brand `telayd` + GitHub `sana-lazystar/telayd`
6-7. Monorepo + L0 빌드 패키지 2개
8-10. Tauri+Rust+TS / CF Tunnel / WebSocketOnly
11. FSL-1.1-MIT
12. IPC 채널 분리
13. Hook Installer global lock
14. Pairing token 32-byte
15. Acceptance 21개 (Anthropic 4 wedge 검증 핵심)
16. JARFIS v4 workflow 빌드 위임
