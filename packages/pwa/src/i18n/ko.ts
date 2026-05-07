/**
 * Korean locale strings (FE-pwa-6)
 * All user-facing text is defined here — no hardcoded strings in components
 */

export const ko = {
  // Pairing screen
  pairing: {
    title: 'Telayd',
    subtitle: 'Claude Code 모바일 연결',
    urlLabel: '터널 URL',
    urlPlaceholder: 'https://random.trycloudflare.com',
    tokenLabel: '페어링 토큰',
    tokenPlaceholder: '43자 토큰을 입력하세요',
    connectButton: '연결',
    connecting: '연결 중...',
    lastUrl: '마지막 URL',
    urlError: '올바른 trycloudflare.com URL을 입력하세요',
    tokenError: '토큰은 영문/숫자/하이픈/언더스코어 43자여야 합니다',
    autoDetected: 'URL에서 자동 감지됨',
    // IG2: replaced by another client
    replaced: '다른 기기에서 연결되어 현재 연결이 종료되었습니다.',
  },

  // Connection states
  connecting: {
    title: '연결 중',
    message: '데몬에 연결하는 중입니다...',
  },

  idle: {
    title: '대기 중',
    message: 'Claude Code에서 질문이 발생하면 여기에 표시됩니다.',
    connectedAs: '연결됨',
    mode: '권한 모드',
    changeMode: '모드 변경',
  },

  disconnected: {
    title: '연결 끊김',
    message: '데몬과 연결이 끊어졌습니다. 재연결 시도 중...',
    retrying: '재연결 중 ({attempt}번째 시도)',
    reconnectNow: '지금 재연결',
  },

  pairingError: {
    title: '페어링 오류',
    tokenMismatch: '토큰이 일치하지 않습니다. 데몬에서 토큰을 확인하세요.',
    // IG4: 'expired' gets its own message (was incorrectly mapped to tokenMismatch)
    expired: '페어링 토큰이 만료되었습니다. 데몬을 재시작하거나 새 토큰을 생성하세요.',
    // IG4: 'replaced' = another device connected and took over this slot
    replaced: '다른 기기에서 연결되어 현재 연결이 종료되었습니다.',
    connectionFailed: '30초 동안 연결에 실패했습니다.',
    repairButton: '다시 페어링',
  },

  // Choice screen
  choice: {
    headerPrefix: '',
    sendButton: '전송',
    sending: '전송 중...',
    cancelButton: '취소',
    freeTextPlaceholder: '직접 입력하세요 (최대 4096자)',
    freeTextToggle: '직접 입력',
    timeout: '응답 시간이 초과되었습니다. 다시 시도합니다...',
    retrying: '재시도 중...',
    injected: '응답이 주입되었습니다',
    // IG1: typed error reason keys (replaces generic {reason} interpolation)
    error: {
      sendKeysFailed: '키 입력 전송에 실패했습니다. 터미널을 확인하세요.',
      dialogNotReady: '다이얼로그가 준비되지 않았습니다. 잠시 후 다시 시도하세요.',
      inquiryStale: '요청이 만료되었습니다.',
      validation: '잘못된 응답 형식입니다.',
    },
    multiSelectWarning: '복수 선택 질문은 수동으로 응답해주세요',
  },

  // Permission mode
  permissionMode: {
    title: '권한 모드',
    subtitle: '현재 Claude Code 실행 모드를 변경합니다',
    plan: 'Plan 모드',
    planDesc: '파일 편집 없이 계획만 수립',
    acceptEdits: 'Accept Edits 모드',
    acceptEditsDesc: '파일 편집을 자동으로 허용',
    default: '기본 모드',
    defaultDesc: '매번 확인하며 진행',
    applyButton: '적용',
    applying: '적용 중...',
    applied: '적용 완료',
    timeout: '응답이 없습니다. 다시 시도합니다...',
    // IG2: rewritten — was "현재 모드는 변경할 수 없습니다." (incorrect: no-session state is deferred, not unsupported)
    notSupported: 'Claude Code prompt가 도착하면 자동 적용됩니다',
    // IG2: new key — shown in prompt-choice when mode toggle was in-flight on INQUIRY_PUSH
    deferredApply: '모드 변경이 대기 중입니다. 다음 prompt에 자동 적용됩니다.',
    cancelButton: '취소',
  },

  // Mode indicator labels
  modeLabel: {
    plan: 'Plan',
    'accept-edits': 'Accept Edits',
    default: '기본',
  },

  // Common
  common: {
    error: '오류',
    loading: '로딩 중...',
    retry: '다시 시도',
    close: '닫기',
  },
} as const

export type Translations = typeof ko
export type TranslationKey = keyof typeof ko
