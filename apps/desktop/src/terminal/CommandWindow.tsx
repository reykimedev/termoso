import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import {
  Box,
  Button,
  FormControlLabel,
  IconButton,
  InputBase,
  ListSubheader,
  MenuItem,
  Select,
  Switch,
  Tooltip,
} from "@mui/material";
import CloseRoundedIcon from "@mui/icons-material/CloseRounded";
import SendRoundedIcon from "@mui/icons-material/SendRounded";
import { useHosts } from "@/ipc/hooks";
import { monoFontFamily } from "@/theme/theme";
import { tr } from "@/i18n";
import {
  commandInput,
  hostGroupPaths,
  liveGroups,
  tagGroupNames,
  type CommandTarget,
} from "./commandTargets";
import {
  commandTargetPanes,
  paneHostInfo,
  sendToTarget,
  setCommandTarget,
  setCommandWindowOpen,
  setHostInfo,
  useTerminal,
} from "./store";

const HISTORY_KEY = "labterm.commandWindow.history";
const HISTORY_MAX = 100;

function loadHistory(): string[] {
  try {
    const v: unknown = JSON.parse(localStorage.getItem(HISTORY_KEY) ?? "[]");
    return Array.isArray(v) ? v.filter((x): x is string => typeof x === "string") : [];
  } catch {
    return [];
  }
}

function saveHistory(list: string[]) {
  try {
    localStorage.setItem(HISTORY_KEY, JSON.stringify(list.slice(-HISTORY_MAX)));
  } catch {
    // history is a convenience; a blocked storage just forgets it
  }
}

/** Keys typed in "send keys as I type" mode, as the terminal would send them. */
function keyData(e: KeyboardEvent): string | null {
  if (e.ctrlKey && !e.altKey && e.key.length === 1) {
    const c = e.key.toUpperCase().charCodeAt(0);
    return c >= 64 && c <= 95 ? String.fromCharCode(c - 64) : null;
  }
  switch (e.key) {
    case "Enter":
      return "\r";
    case "Backspace":
      return "\x7f";
    case "Tab":
      return "\t";
    case "Escape":
      return "\x1b";
    case "ArrowUp":
      return "\x1b[A";
    case "ArrowDown":
      return "\x1b[B";
    case "ArrowRight":
      return "\x1b[C";
    case "ArrowLeft":
      return "\x1b[D";
    default:
      return e.key.length === 1 && !e.metaKey ? e.key : null;
  }
}

function targetKey(t: CommandTarget): string {
  switch (t.kind) {
    case "group":
      return `group:${t.id}`;
    case "tag":
      return `tag:${t.tag}`;
    case "hostGroup":
      return `hostGroup:${JSON.stringify(t.path)}`;
    default:
      return t.kind;
  }
}

function parseTargetKey(key: string): CommandTarget {
  if (key.startsWith("group:")) return { kind: "group", id: key.slice(6) };
  if (key.startsWith("tag:")) return { kind: "tag", tag: key.slice(4) };
  if (key.startsWith("hostGroup:")) {
    const path: unknown = JSON.parse(key.slice(10));
    if (Array.isArray(path)) return { kind: "hostGroup", path: path.map(String) };
  }
  if (key === "active" || key === "visible") return { kind: key };
  return { kind: "all" };
}

/**
 * Docked under the terminals: type a command once and send it to this
 * session, the tab on screen, every session, a tab group or a tag group.
 * Each session gets a line once, however many of the chosen groups it is in.
 */
export function CommandWindow() {
  const open = useTerminal((s) => s.commandWindow.open);
  const target = useTerminal((s) => s.commandWindow.target);
  const tabs = useTerminal((s) => s.tabs);
  const groups = useTerminal((s) => s.tabGroups);
  const panes = useTerminal((s) => s.panes);
  const hosts = useHosts(null);
  const [text, setText] = useState("");
  const [live, setLive] = useState(false);
  const [history, setHistory] = useState<string[]>(loadHistory);
  const [cursor, setCursor] = useState<number | null>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);

  // Tag and host group targets read the hosts' tags and groups from the store.
  useEffect(() => {
    setHostInfo(
      new Map((hosts.data ?? []).map((h) => [h.id, { tags: h.tags, groupPath: h.groupPath }])),
    );
  }, [hosts.data]);

  useEffect(() => {
    if (open) inputRef.current?.focus();
  }, [open]);

  const groupList = useMemo(
    () => Object.values(liveGroups(groups, tabs)).sort((a, b) => a.name.localeCompare(b.name)),
    [groups, tabs],
  );
  // Automatic targets: the tags and host groups of the open sessions, parents included.
  const { tagList, hostGroupList } = useMemo(() => {
    const s = { tabs, panes } as Parameters<typeof paneHostInfo>[1];
    const infos = tabs.flatMap((t) => t.paneIds).map((id) => paneHostInfo(id, s));
    return {
      tagList: tagGroupNames(infos.flatMap((i) => [...i.tags])),
      hostGroupList: hostGroupPaths(infos.map((i) => i.groupPath)),
    };
    // hosts.data: tags and groups change when the host list reloads
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tabs, panes, hosts.data]);
  const count = useTerminal((s) => commandTargetPanes(target, s).length);

  if (!open) return null;

  const send = () => {
    if (!text.trim()) return;
    sendToTarget(target, commandInput(text));
    const next = [...history.filter((h) => h !== text), text].slice(-HISTORY_MAX);
    setHistory(next);
    saveHistory(next);
    setText("");
    setCursor(null);
  };

  const onKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Escape" && !live) {
      setCommandWindowOpen(false);
      return;
    }
    if (live) {
      const data = keyData(e);
      if (data !== null) {
        e.preventDefault();
        sendToTarget(target, data);
      }
      return;
    }
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      send();
      return;
    }
    // Up / down walk the window's own history while the input is one line.
    if ((e.key === "ArrowUp" || e.key === "ArrowDown") && !text.includes("\n")) {
      if (history.length === 0) return;
      e.preventDefault();
      const at = cursor ?? history.length;
      const nextAt = e.key === "ArrowUp" ? Math.max(0, at - 1) : Math.min(history.length, at + 1);
      setCursor(nextAt === history.length ? null : nextAt);
      setText(history[nextAt] ?? "");
    }
  };

  return (
    <Box
      role="region"
      aria-label={tr("Command window")}
      sx={{
        display: "flex",
        alignItems: "flex-start",
        gap: 1,
        px: 1.25,
        py: 0.75,
        borderTop: 1,
        borderColor: "border.light",
        bgcolor: "surface.base",
        flexWrap: "wrap",
      }}
    >
      <Select
        size="small"
        value={targetKey(target)}
        onChange={(e) => setCommandTarget(parseTargetKey(e.target.value))}
        aria-label={tr("Send to")}
        sx={{ minWidth: 180, maxWidth: 260, fontSize: 13 }}
      >
        <MenuItem value="active">{tr("This session")}</MenuItem>
        <MenuItem value="visible">{tr("Visible sessions")}</MenuItem>
        <MenuItem value="all">{tr("All sessions")}</MenuItem>
        {groupList.length > 0 && <ListSubheader>{tr("Tab groups")}</ListSubheader>}
        {groupList.map((g) => (
          <MenuItem key={g.id} value={`group:${g.id}`}>
            <Box
              component="span"
              sx={{ width: 10, height: 10, borderRadius: "50%", bgcolor: g.color, mr: 1 }}
            />
            {g.name}
          </MenuItem>
        ))}
        {hostGroupList.length > 0 && <ListSubheader>{tr("Host groups")}</ListSubheader>}
        {hostGroupList.map((p) => (
          <MenuItem
            key={JSON.stringify(p)}
            value={`hostGroup:${JSON.stringify(p)}`}
            sx={{ pl: 2 + (p.length - 1) * 1.5 }}
          >
            {p[p.length - 1]}
          </MenuItem>
        ))}
        {tagList.length > 0 && <ListSubheader>{tr("Tags")}</ListSubheader>}
        {tagList.map((t) => (
          <MenuItem key={t} value={`tag:${t}`} sx={{ pl: 2 + (t.split(".").length - 1) * 1.5 }}>
            #{t}
          </MenuItem>
        ))}
      </Select>
      <InputBase
        inputRef={inputRef}
        multiline
        maxRows={6}
        value={live ? "" : text}
        placeholder={
          live
            ? tr("Keys go straight to {count} sessions", { count })
            : tr("Type a command, Enter sends it, Shift+Enter adds a line")
        }
        onChange={(e) => {
          if (!live) setText(e.target.value);
        }}
        onKeyDown={onKeyDown}
        sx={{
          flex: "1 1 240px",
          minWidth: 0,
          fontFamily: monoFontFamily,
          fontSize: 13,
          px: 1,
          py: 0.5,
          borderRadius: 1,
          bgcolor: "surface.highest",
        }}
        inputProps={{ "aria-label": tr("Command") }}
      />
      <Button
        variant="contained"
        size="small"
        startIcon={<SendRoundedIcon />}
        disabled={live || count === 0 || !text.trim()}
        onClick={send}
        sx={{ whiteSpace: "nowrap" }}
      >
        {count === 1 ? tr("Send") : tr("Send to {count}", { count })}
      </Button>
      <FormControlLabel
        control={<Switch size="small" checked={live} onChange={(e) => setLive(e.target.checked)} />}
        label={tr("Send keys as I type")}
        slotProps={{ typography: { variant: "body2" } }}
        sx={{ mr: 0 }}
      />
      <Tooltip title={tr("Close")}>
        <IconButton
          size="small"
          aria-label={tr("Close")}
          onClick={() => setCommandWindowOpen(false)}
        >
          <CloseRoundedIcon fontSize="small" />
        </IconButton>
      </Tooltip>
    </Box>
  );
}
