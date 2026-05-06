---
type: meeting-summary
idea: "Telayd L0 데몬 prototype — 본인 mac 1대에서 매일 쓰는 working spike"
meeting_name: "telayd-l0-kickoff"
date: "2026-05-06"
participants: [PO, TL, owner (이산하/sana-lazystar)]
status: completed
---

# telayd-l0-kickoff Meeting Summary

## One-Line Summary

Telayd L0 (본인 mac 1대 dogfooding spike)을 사업화 NO + cross-CLI wedge 전제로 lock. 21개 acceptance criteria + 10개 in-scope module + dependency graph 5-layer 정의. JARFIS v4 workflow로 빌드 위임 — `~/AI-OVERALL/telayd/` (sana-lazystar/telayd, FSL-1.1-MIT). SSOT는 `~/AI--SPILL-OVER/telayd-planning/mvp-scope/scope.md` v0.2.3 (5-iteration audit cycle CONVERGED).

## Key Decisions

1. **사업화 NO** (b-4 결정) — dogfooding-driven OSS only. JARFIS testbed로서의 가치 우선. 회사로서 비전 약함 + customer development 5명 인터뷰 못 잡음 + GTM 견디는 편 not 즐기는 편.
2. **L0 = 본인 working spike** (시간 무제한 / 완성도 최우선) — Claude Code only / sentinel parser stub / Desktop GUI 없음 / hard-coded pairing token / quick CF tunnel. 21개 acceptance criteria 모두 충족 시 done.
3. **Wedge 명확화** — Anthropic Remote Control이 Claude Code mobile attach commodity화 (12.48M MAU 번들). 진짜 durable wedge = **cross-CLI 통합** (1st-party 절대 안 만듦, 18-36+ months defensible) + **permission UX 정확도** (Anthropic 4 GitHub issues #29214/#29438/#34581/#35637 — 3-6 month expiry).

## Next Steps

- `/jarfis:work L0 데몬 prototype 본격 구현 — packages/daemon (Rust) + packages/pwa (Vite). hook installer + tmux controller + WebSocket bridge + CF Tunnel manager + PWA UI(페어링+choice). --meeting telayd-l0-kickoff` 호출
- L0 빌드 패키지: `packages/daemon/` + `packages/pwa/` (다른 패키지는 L1+ 진입 시 생성)
- Phase 1a 답안 lock: `design.mode = text`, `responsive = pc-mobile`, `api.mode = null`, `devops = false`
- Brand asset lock (도메인 telayd.dev / Homebrew tap)은 L1 진입 prerequisite로 deferred
