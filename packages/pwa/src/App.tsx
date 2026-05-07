/**
 * App.tsx — SPA root + state machine (FE-pwa-4)
 * 7-page state routing via useReducer (no router library)
 * WsClient callbacks bridge to app state dispatch
 */

import { useCallback, useEffect, useReducer } from 'react'
import type {
  ModeToggleAckPayload,
  PairingAckPayload,
  PairingRejectPayload,
  PermissionMode,
} from './lib/protocol'
import type { InquiryPushPayload } from './lib/protocol'
import { clearStorage } from './lib/storage'
import { wsClient } from './lib/ws-client'
import type { StatusChangeEvent, WsStatus } from './lib/ws-client'
import { appReducer, initialState } from './state/app-state'
import type { AppAction } from './state/app-state'
import { ConnectingView } from './views/ConnectingView'
import { DisconnectedView } from './views/DisconnectedView'
import { IdleView } from './views/IdleView'
import { PairingErrorView } from './views/PairingErrorView'
import { PairingView } from './views/PairingView'
import { PermissionModeView } from './views/PermissionModeView'
import { PromptChoiceView } from './views/PromptChoiceView'

export function App() {
  const [state, dispatch] = useReducer(appReducer, initialState)

  // Wire WsClient callbacks → app state dispatch
  // IG10: receives (status, event) tuple; _event available for future instrumentation
  const handleStatusChange = useCallback((status: WsStatus, _event: StatusChangeEvent) => {
    switch (status) {
      case 'reconnecting':
        dispatch({ type: 'WS_RECONNECTING', attempt: wsClient.getReconnectAttempt() })
        break
      // IG3: dispatch WS_DISCONNECTED on server-shutdown (4003)
      case 'disconnected':
        dispatch({ type: 'WS_DISCONNECTED' })
        break
      case 'pairing-error':
        dispatch({ type: 'WS_TIMEOUT' })
        break
      default:
        // 'idle', 'connecting', 'connected' — handled via pairing callbacks
        break
    }
  }, [])

  const handlePairingAck = useCallback((payload: PairingAckPayload) => {
    dispatch({
      type: 'PAIRING_ACK',
      sessionId: payload.session,
      serverVersion: payload.server_version,
    })
  }, [])

  const handlePairingReject = useCallback((payload: PairingRejectPayload) => {
    dispatch({ type: 'PAIRING_REJECT', reason: payload.reason })
  }, [])

  const handleInquiryPush = useCallback((payload: InquiryPushPayload) => {
    dispatch({ type: 'INQUIRY_PUSH', payload })
  }, [])

  const handleModeToggleAck = useCallback((payload: ModeToggleAckPayload) => {
    if (payload.applied) {
      dispatch({ type: 'MODE_UPDATED', mode: payload.mode })
    }
  }, [])

  // Register callbacks — ack/error are handled inside view components directly
  useEffect(() => {
    wsClient.setCallbacks({
      onStatusChange: handleStatusChange,
      onPairingAck: handlePairingAck,
      onPairingReject: handlePairingReject,
      onInquiryPush: handleInquiryPush,
      onModeToggleAck: handleModeToggleAck,
    })
    return () => {
      wsClient.setCallbacks({})
    }
  }, [
    handleStatusChange,
    handlePairingAck,
    handlePairingReject,
    handleInquiryPush,
    handleModeToggleAck,
  ])

  // Handlers
  function handleConnect(url: string, token: string) {
    dispatch({ type: 'CONNECT_START', tunnelUrl: url })
    wsClient.connect(url, token)
  }

  function handleReconnect() {
    // IG3: disconnect ws before resetting (mirror handleRepair symmetry)
    wsClient.disconnect()
    dispatch({ type: 'RESET_TO_PAIRING' })
  }

  function handleInquiryResolved() {
    dispatch({ type: 'INQUIRY_RESOLVED' })
  }

  function handleOpenModeToggle() {
    dispatch({ type: 'OPEN_PERMISSION_MODE' })
  }

  function handleModeApplied(mode: PermissionMode) {
    dispatch({ type: 'MODE_UPDATED', mode })
  }

  function handleModeCancel() {
    dispatch({ type: 'CLOSE_PERMISSION_MODE' })
  }

  function handleRepair() {
    wsClient.disconnect()
    // IG11: clearStorage on repair — flush stale lastUrl/masked-token so user starts fresh
    clearStorage()
    dispatch({ type: 'RESET_TO_PAIRING' })
  }

  const { page } = state

  return (
    <div className="app-container">
      {page === 'pairing' && <PairingView onConnect={handleConnect} loading={false} />}

      {page === 'connecting' && <ConnectingView />}

      {page === 'idle' && (
        <IdleView
          permissionMode={state.permissionMode}
          serverVersion={state.serverVersion ?? undefined}
          onOpenModeToggle={handleOpenModeToggle}
        />
      )}

      {page === 'prompt-choice' && state.activeInquiry && (
        <PromptChoiceView
          inquiry={state.activeInquiry}
          permissionMode={state.permissionMode}
          onResolved={handleInquiryResolved}
          onOpenModeToggle={handleOpenModeToggle}
          // IG2: notify PromptChoiceView that a mode toggle was abandoned mid-flight
          modeAbandonedToast={state.pendingModeAbandoned}
          onClearModeAbandonedToast={() => dispatch({ type: 'CLEAR_MODE_ABANDONED' } as AppAction)}
        />
      )}

      {page === 'permission-mode' && (
        <PermissionModeView
          current={state.permissionMode}
          onApplied={handleModeApplied}
          onCancel={handleModeCancel}
        />
      )}

      {page === 'disconnected' && (
        <DisconnectedView attempt={state.reconnectAttempt} onReconnect={handleReconnect} />
      )}

      {page === 'pairing-error' && (
        <PairingErrorView reason={state.pairingErrorReason} onRepair={handleRepair} />
      )}
    </div>
  )
}
