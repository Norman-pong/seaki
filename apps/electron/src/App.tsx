import { useState, useEffect } from "react";
import { Panel, Group, Separator, usePanelRef } from "react-resizable-panels";

import { TitleBar } from "@/components/TitleBar";
import { SessionSidebar } from "@/components/SessionSidebar";
import { ChatPanel } from "@/components/ChatPanel";
import { WikiSidebar } from "@/components/WikiSidebar";

import { createChatSessions, createInitialSession } from "@/models/chatModel";
import { createWikiTree } from "@/models/wikiTreeModel";
import type { ChatSession } from "@/models/chatModel";

const wikiTree = createWikiTree();

import {
  createElectronAppModel,
  type ApprovalDiffModel,
} from "./appModel";

import "./styles.css";

export function App() {
  const [sessions, setSessions] = useState<readonly ChatSession[]>(() => createChatSessions());
  const [activeSessionId, setActiveSessionId] = useState<string>(() => createInitialSession().id);
  const [selectedPageId, setSelectedPageId] = useState<string>("wiki_m0_ingest");
  const [approval, setApproval] = useState<ApprovalDiffModel | null>(null);
  const [leftCollapsed, setLeftCollapsed] = useState(false);
  const [rightCollapsed, setRightCollapsed] = useState(false);
  const leftPanelRef = usePanelRef();
  const rightPanelRef = usePanelRef();

  useEffect(() => {
    let active = true;

    void createElectronAppModel().then((model) => {
      if (active) {
        setApproval(model.approval);
      }
    });

    return () => {
      active = false;
    };
  }, []);

  const activeSession = sessions.find((s) => s.id === activeSessionId) ?? sessions[0];

  function handleSelectSession(sessionId: string) {
    setActiveSessionId(sessionId);
    setSessions((prev) =>
      prev.map((s) => ({ ...s, active: s.id === sessionId })),
    );
  }

  function handleNewSession() {
    const newSession: ChatSession = {
      id: `session_${Date.now()}`,
      title: "新会话",
      timestamp: new Date().toISOString(),
      active: true,
      messages: [],
    };
    setSessions((prev) => [newSession, ...prev.map((s) => ({ ...s, active: false }))]);
    setActiveSessionId(newSession.id);
  }

  function handleDeleteSession(sessionId: string) {
    const next = sessions.filter((s) => s.id !== sessionId);
    if (next.length === 0) {
      const fallback: ChatSession = {
        id: `session_${Date.now()}`,
        title: "新会话",
        timestamp: new Date().toISOString(),
        active: true,
        messages: [],
      };
      setSessions([fallback]);
      setActiveSessionId(fallback.id);
      return;
    }
    setSessions(next);
    if (activeSessionId === sessionId) {
      setActiveSessionId(next[0]!.id);
    }
  }

  function toggleLeftPanel() {
    const next = !leftCollapsed;
    setLeftCollapsed(next);
    if (next) {
      leftPanelRef.current?.resize(0);
    } else {
      leftPanelRef.current?.resize("18%");
    }
  }

  function toggleRightPanel() {
    const next = !rightCollapsed;
    setRightCollapsed(next);
    if (next) {
      rightPanelRef.current?.resize(0);
    } else {
      rightPanelRef.current?.resize("32%");
    }
  }

  return (
    <div className="app-shell">
      <TitleBar
        session={activeSession}
        leftCollapsed={leftCollapsed}
        onToggleLeft={toggleLeftPanel}
        rightCollapsed={rightCollapsed}
        onToggleRight={toggleRightPanel}
      />
      <div className="app-body">
        <Group orientation="horizontal" className="app-panels">
          {/* Left: Session Sidebar */}
          <Panel
            defaultSize="18%"
            minSize="14%"
            maxSize="28%"
            className="app-left-panel"
            panelRef={leftPanelRef}
            style={{ transition: "flex-basis 0.3s ease, flex-grow 0.3s ease" }}
          >
            <SessionSidebar
              sessions={sessions}
              activeSessionId={activeSessionId}
              onSelectSession={handleSelectSession}
              onNewSession={handleNewSession}
              onDeleteSession={handleDeleteSession}
              isCollapsed={leftCollapsed}
            />
          </Panel>

          <Separator className="app-resize-handle" />

          {/* Center: Chat Panel */}
          <Panel defaultSize="50%" minSize="30%" className="app-center-panel">
            {activeSession ? <ChatPanel session={activeSession} /> : null}
          </Panel>

          <Separator className="app-resize-handle" />

          {/* Right: Wiki Sidebar + Approval */}
          <Panel
            defaultSize="32%"
            minSize="22%"
            maxSize="45%"
            className="app-right-panel"
            panelRef={rightPanelRef}
            style={{ transition: "flex-basis 0.3s ease, flex-grow 0.3s ease" }}
          >
            <WikiSidebar
              tree={wikiTree}
              selectedPageId={selectedPageId}
              onSelectPage={setSelectedPageId}
              approval={approval}
              onApprovalChange={setApproval}
              isCollapsed={rightCollapsed}
            />
          </Panel>
        </Group>
      </div>
    </div>
  );
}
