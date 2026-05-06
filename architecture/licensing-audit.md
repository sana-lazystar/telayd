# Licensing Audit — Telayd 의존성 + 자체 라이선스

> 작성일: 2026-05-04
> 상태: **lock — Telayd 자체 라이선스 = FSL (b-3 결정)**
> 검증 시점: v1 public release 직전 한 번 더 audit

---

## 1. Telayd 자체 라이선스 — **FSL** (Functional Source License)

### 결정 (b-3 §라이선스 결정)
- **Telayd는 FSL-1.1-MIT로 release** (2년 후 자동 MIT 전환)
- 이유:
  - 소스코드 100% 공개 → portfolio/평판 효과 유지
  - 첫 2년: "Telayd와 경쟁하는 SaaS 만들기"만 차단 → 사업화 가능성 보존
  - 2년 후 자동으로 MIT 전환 → 커뮤니티 친화 (영구 락인 아님)
  - HashiCorp(Vault), Sentry, MariaDB 검증 패턴

### 사용자에게 무엇을 의미하나
- 개인 사용자가 본인 작업에 사용 → ✓ 무료
- 회사가 사내 도구로 사용 → ✓ 무료
- 다른 프로젝트에 통합 → ✓ 무료 (그 프로젝트가 Telayd 경쟁이 아니면)
- 컨설팅 회사가 고객사에 설치/관리 → ✓ 무료
- "Telayd Cloud Pro"같은 직접 경쟁 SaaS 만들기 → ✗ 차단 (2년간)

### Repo / LICENSE 파일
- v1 release 직전 LICENSE 파일 추가
- README에 라이선스 배지 표시
- 의존성 NOTICE 파일 별도 (각 의존성 라이선스 attribution)

### 폴백 옵션
- FSL이 채택률 낮아 community 작으면 → MIT로 변경 가능 (단 단방향, MIT 변경 후 다시 FSL 못 감)
- 사업화 강력 의지 시 → BSL 또는 proprietary로 변경 가능

---

## 2. 의존성별 라이선스 (그린 — 자유 상업 사용 OK)

| 의존성 | 라이선스 | 용도 | Notes |
|---|---|---|---|
| **tmux** | ISC (BSD-like) | 세션 호스팅 인프라 | 매우 permissive, 모든 use case OK |
| **Tauri** | Apache 2.0 / MIT dual | Desktop app shell | dual license, 자유 |
| **React** | MIT | Frontend framework | 가장 친숙 |
| **React Native** (v2+) | MIT | Mobile native app | 자유 |
| **Vite** | MIT | PWA build tool | 자유 |
| **TypeScript** | Apache 2.0 | 타입 시스템 | 자유 |
| **Rust + crates 대부분** | MIT/Apache 2.0 dual | Daemon backend 언어 | 표준 dual license |
| **Cloudflare Pages** (정적 호스팅) | Cloudflare ToS | PWA hosting | Free tier 100GB/월, 상업 OK |
| **GitHub Pages** (정적 호스팅 alt) | GitHub ToS | PWA hosting alt | 비상업 권장 (모호), 상업도 가능 |
| **GitHub Releases** (binary 배포) | GitHub ToS | Telayd binary 배포 | 무료/무제한 |
| **Homebrew** (macOS 배포) | BSD-2 + 정책 | Telayd Tap 배포 | 자유 |
| **WebSocket / unix socket / etc.** | 표준 protocol | Internal IPC | n/a |

→ 위 모든 의존성 = **FSL Telayd와 호환, 사업화 OK.**

---

## 3. 의존성 — 노란 (확인/주의 필요)

### 3.1 Cloudflare Tunnel (`cloudflared`)

**의존성:** Apache 2.0 오픈소스 binary ✓
**서비스:** Cloudflare ToS 적용

| 시나리오 | OK / 주의 |
|---|---|
| Telayd가 cloudflared binary를 사용자 mac에 설치/실행 | ✓ OK (Apache 2.0) |
| 사용자가 본인 cloudflared로 본인 모바일 → 본인 데스크탑 라우팅 | ✓ OK (Free tier 100GB/월) |
| Telayd 배포물 안에 cloudflared binary 포함 | ⚠ NOTICE 파일 필요 (Apache 2.0 attribution), recommended: 사용자 시스템에 직접 install하게 함 |
| Telayd Cloud (운영자 hosted relay) — Cloudflare Workers/Tunnel 사용 | ⚠ Cloudflare ToS Section 2.8 검토 — 일반 트래픽 OK, large scale은 paid plan 강제 가능 |

**v1 결정 사항:**
- cloudflared는 **사용자 mac에 직접 install** (Telayd가 자동 download/install) — Apache 2.0 attribution NOTICE에 추가
- Telayd Cloud (Phase 1+) 진입 시 Cloudflare Business 정책 재검토

### 3.2 Anthropic Claude Code

**상태:** Anthropic의 proprietary CLI tool
**Telayd와의 관계:** 사용자가 본인 라이선스로 Claude Code 사용 → Telayd는 단지 그 세션을 attach해서 외부 routing

| 시나리오 | OK / 주의 |
|---|---|
| Telayd가 Claude Code의 hook/output을 가로채서 routing | ⚠ Anthropic AUP 검토 필요 |
| Telayd가 Claude Code binary를 재배포 | ✗ 절대 금지 (closed source) |
| Telayd의 docs/marketing이 "Bypass Claude's interaction" 언급 | ⚠ marketing 표현 주의 |

**v1 결정 사항:**
- Anthropic Acceptable Use Policy (https://www.anthropic.com/legal/aup) 정독 + Telayd 사용 case 적합성 확인
- Marketing은 "Telayd routes Claude's user-facing prompts to your mobile" 같은 중립적 표현
- Claude Code 자체는 사용자가 본인 책임으로 install (Telayd가 bundle 안 함)

### 3.3 Google Gemini CLI

**상태:** Google의 도구, Gemini Pro 가입자 사용
**Telayd와의 관계:** 위 Claude Code와 동일 패턴

**v1 결정 사항:** 위와 동일 — Google ToS 정독, Gemini CLI 자체는 bundle 안 함, 사용자가 본인 install

---

## 4. 의존성 — 레드 (피하거나 신중)

### 4.1 Tailscale — 다행히 채택 안 함 ✓

**상태:** Free tier "Personal Use" 제한 — 상업/팀 사용은 Business Plan ($6/user/월)

만약 Telayd가 Tailscale을 hard dependency로 했다면:
- 사용자 사업 사용 시 Tailscale 별도 비용 발생 → 마찰
- Telayd가 사용자에게 "Tailscale 사용해" 권장은 OK (사용자 책임)
- 단 Telayd Cloud가 Tailscale 의존 시 운영자 비용 ↑

**b-3 결정으로 회피됨:** transport는 Cloudflare Tunnel 채택. Tailscale은 spec에 alternative로만 backup. 라이선스 위험 zero.

---

## 5. Privacy / Data Plane 정책

### 5.1 데이터 흐름과 우려
- 사용자의 LLM 대화 내용 (질문/응답)이 cloudflared → Cloudflare 인프라 → 사용자 모바일로 전송
- Cloudflare ToS: 트래픽을 라우팅하지만 **저장하지 않음** (정책상)
- 단 사용자에게 "Cloudflare를 거침" 명시 필요 (Privacy Policy)

### 5.2 권장 — E2E 암호화

**Telayd 데몬과 사용자 모바일 사이 E2E 암호화 layer 추가:**
- 페어링 시 데몬 ↔ 모바일 사이 shared secret 교환 (QR 스캔)
- 이후 모든 메시지 페이로드가 데몬에서 암호화, 모바일에서 복호화
- Cloudflare는 암호화된 페이로드만 봄 (그냥 transport)

→ 보안 + privacy + 의료/민감 데이터 사용자 보호.

### 5.3 Privacy Policy (v1 release 시 작성)
- "Telayd는 데몬 ↔ 모바일 사이 메시지를 Cloudflare를 통해 라우팅합니다"
- "메시지는 E2E 암호화되어 Cloudflare는 페이로드를 볼 수 없습니다"
- "Telayd 운영자(대표)는 사용자 데이터에 접근하지 않습니다 (셀프호스팅 모델)"
- "Telayd Cloud (Phase 1+ paid)는 별도 Privacy Policy 적용"

---

## 6. 한국 법령 — **N/A** (b-4 사업화 NO 결정)

> ~~b-3 phase 모델~~ → b-4에서 **사업화 NO** 결정. 결제/법령 조항 모두 무관.

| 항목 | b-4 후 상태 |
|---|---|
| 통신판매업 신고 | ❌ N/A (결제 시작 안 함) |
| 개인정보 처리방침 작성 | ⚠️ 권장 (L2 = public OSS release 시 Privacy Policy 작성, 결제와 무관) |
| 약관 (이용약관/환불 정책) | ❌ N/A (결제 없음) |
| 카드 결제 PG 연동 | ❌ N/A |
| 사업자등록 | ❌ N/A |
| B2B 계약/SLA | ❌ N/A |

→ **OSS only로만 진행**. 셀프호스팅 모델 (L2 hosted relay alt)에서도 Telayd 운영자가 사용자 데이터 처리 안 함.

---

## 7. L2 (Public OSS Release) 직전 Audit Checklist

> b-4 정정: 기존 "v1 release"는 L2 진입 시점과 동일 (`mvp-scope/scope.md` §5.2 epic 8). 세 문서 (scope / spec / licensing-audit) align됨.

다음 항목 모두 ✓ 후 public release:

- [ ] LICENSE 파일 (FSL-1.1-MIT) repo root에
- [ ] NOTICE 파일 — 모든 dependency 라이선스 attribution (cloudflared Apache 2.0 포함)
- [ ] README에 라이선스 배지 + 사용 조건 1줄 요약
- [ ] cloudflared binary 자동 install 시 사용자에게 Cloudflare ToS link 표시
- [ ] Anthropic AUP 정독 + Telayd use case 적합성 확인 (legal review 권장)
- [ ] Google ToS 동일
- [ ] Privacy Policy (간단 버전이라도) 사이트에 게시
- [ ] **E2E 암호화 구현** (보안 + privacy) — `scope.md` §6.11 / spec §11.4 align — L2 prerequisite
- [ ] Cloudflare ToS Section 2.8 (acceptable content) 검토

---

## 8. 결정 요약 (b-4 align)

- **Telayd 자체:** FSL-1.1-MIT (소스 공개 + 경쟁 차단 + 2년 후 MIT 자동)
- **의존성:** 모두 호환 (tmux/Tauri/React/Rust 등 표준 OSS)
- **CF Tunnel:** Apache 2.0 + Cloudflare ToS 준수, 사용자 셀프호스팅 모델로 운영자 부담 없음
- **Claude/Gemini CLI:** 사용자가 본인 책임으로 install, Telayd는 bundle 안 함
- **Privacy:** E2E 암호화 = **L2 = public OSS release prerequisite** (scope.md §6.11 + spec §11.4 align)
- **사업화:** ❌ NO (b-4) — OSS only, 한국 법령 진입 사항 N/A
