import { initWorkspaceShell, prepareUserSelectedFile } from "@seaki/domain";

export function createElectronAppModel() {
  const workspace = initWorkspaceShell({
    auditHead: "audit:pending-daemon",
    currentRevision: "wiki_rev_0",
    degradedReason: "daemon_recovering",
    rootUri: "file:///workspace",
    workspaceId: "ws_local_preview",
  });
  const preparedImport = prepareUserSelectedFile({
    declaredMime: "text/markdown",
    declaredSize: 0,
    displayName: "等待选择本机文件",
    opaqueFileRef: "electron://selection/pending",
    platform: "electron",
    selectionId: "sel_pending",
  });

  return {
    importStage: preparedImport.state.stage,
    workspaceStage: workspace.state.stage,
    workspaceTitle: workspace.workspaceId,
  };
}
