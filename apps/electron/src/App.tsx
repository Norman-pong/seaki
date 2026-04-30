import { useState, useEffect } from "react";
import { Panel, Group, Separator } from "react-resizable-panels";

import { SessionSidebar } from "@/components/SessionSidebar";
import { ChatPanel } from "@/components/ChatPanel";
import { WikiSidebar } from "@/components/WikiSidebar";
import { ApprovalWidget } from "@/components/ApprovalWidget";

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
  const [sessions, setSessions] = useState<readonly ChatSession[]>(createChatSessions());
  const [activeSessionId, setActiveSessionId] = useState<string>(createInitialSession().id);
  const [selectedPageId, setSelectedPageId] = useState<string>("wiki_m0_ingest");
  const [approval, setApproval] = useState<ApprovalDiffModel | null>(null);

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
    setSessions((prev) => {
      const next = prev.filter((s) => s.id !== sessionId);
      if (next.length === 0) {
        const fallback: ChatSession = {
          id: `session_${Date.now()}`,
          title: "新会话",
          timestamp: new Date().toISOString(),
          active: true,
          messages: [],
        };
        return [fallback];
      }
      if (activeSessionId === sessionId) {
        setActiveSessionId(next[0]!.id);
      }
      return next;
    });
  }

  return (
    <div className="app-shell">
      <Group orientation="horizontal" className="app-panels">
        {/* Left: Session Sidebar */}
        <Panel
          defaultSize={18}
          minSize={14}
          maxSize={28}
          className="app-left-panel"
        >
          <SessionSidebar
            sessions={sessions}
            activeSessionId={activeSessionId}
            onSelectSession={handleSelectSession}
            onNewSession={handleNewSession}
            onDeleteSession={handleDeleteSession}
          />
        </Panel>

        <Separator className="app-resize-handle" />

        {/* Center: Chat Panel */}
        <Panel defaultSize={50} minSize={30} className="app-center-panel">
          {activeSession ? <ChatPanel session={activeSession} /> : null}
        </Panel>

        <Separator className="app-resize-handle" />

        {/* Right: Wiki Sidebar + Approval */}
        <Panel
          defaultSize={32}
          minSize={22}
          maxSize={45}
          className="app-right-panel"
        >
          <WikiSidebar
            tree={wikiTree}
            selectedPageId={selectedPageId}
            onSelectPage={setSelectedPageId}
          />
          {approval ? (
            <ApprovalWidget
              model={approval}
              onChange={setApproval}
            />
          ) : null}
        </Panel>
      </Group>
    </div>
  );
}
