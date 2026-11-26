// The desktop's settings with defaults for the browser: no tray, no local shell, no agent, no
// auto-update. Stored in the encrypted state, so they follow the lock password, not the account.

export function defaultSettings(): Record<string, unknown> {
  return {
    theme: "system", language: "system", hostsView: "grid", forwardingView: "grid", keychainView: "grid",
    terminalFontSize: 14, terminalFontFamily: "JetBrains Mono", terminalLineHeight: 1.0, terminalTheme: "auto",
    cursorBlink: true, cursorStyle: "block", scrollback: 10000, copyOnSelect: false, pasteOnRightClick: false,
    confirmCloseTab: true, confirmPasteMultiline: true, autocomplete: true, shellIntegration: false, triggers: [],
    restoreCommands: "type", tabTitle: "label", terminalBell: false, brightBold: false, termType: "xterm-256color",
    autoReconnect: false, keywordHighlight: true, localShell: "", keepAliveSeconds: 30, detectOs: false,
    postQuantumKex: true, useSshAgent: false, serveSshAgent: false, agentConfirm: true, runAutomations: false,
    recordSessions: false, logRetentionDays: 0, lockAfterMinutes: 15, autostartForwarding: false,
    minimizeToTray: false, syncConflict: "newest_wins", syncIntervalSeconds: 60, uploadLogs: false,
    syncCredentials: true, updateCheck: "manual", updateUrl: "", welcomeSeen: true, notifications: false,
    notifyCommands: false, notifyCommandSeconds: 5, notifyTransfers: false, sftpRateLimitKib: 0,
    notifySessions: false, notifyAccount: false, shortcuts: {}, sftpOpenWith: {}, proxyCommands: {},
  };
}
